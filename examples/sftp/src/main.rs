use std::{
    env,
    ffi::OsStr,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    net::TcpStream,
    path::Path,
    sync::mpsc,
};

use rkyv::{Archive, Deserialize, Serialize};
use ssh2::Sftp;
use thiserror::Error;
use widestring::{u16str, U16String};
use wincs::{
    error::CloudErrorKind,
    filter::{info, ticket, SyncFilter},
    metadata::Metadata,
    placeholder::{ConvertOptions, Placeholder},
    placeholder_file::PlaceholderFile,
    request::Request,
    root::{HydrationType, PopulationType, Registration, SecurityId, Session, SyncRootIdBuilder},
    utility::{FileTime, WriteAt},
};

// max should be 65536, this is done both in term-scp and sshfs because it's the
// max packet size for a tcp connection
const DOWNLOAD_CHUNK_SIZE_BYTES: usize = 65536;
// const UPLOAD_CHUNK_SIZE_BYTES: usize = 4096;

const PROVIDER_NAME: &str = "wincs";
const DISPLAY_NAME: &str = "Sftp";

#[derive(Debug, Archive, Serialize, Deserialize)]
pub struct FileBlob {
    relative_path: String,
}

fn main() {
    let tcp = TcpStream::connect(env::var("SERVER").unwrap()).unwrap();
    let mut session = ssh2::Session::new().unwrap();
    session.set_blocking(true);
    session.set_tcp_stream(tcp);
    session.handshake().unwrap();
    session
        .userauth_password(
            &env::var("USERNAME").unwrap_or_default(),
            &env::var("PASSWORD").unwrap_or_default(),
        )
        .unwrap();

    let sftp = session.sftp().unwrap();

    let sync_root_id = SyncRootIdBuilder::new(U16String::from_str(PROVIDER_NAME))
        .user_security_id(SecurityId::current_user().unwrap())
        .build();

    let client_path = get_client_path();
    if !sync_root_id.is_registered().unwrap() {
        let u16_display_name = U16String::from_str(DISPLAY_NAME);
        Registration::from_sync_root_id(&sync_root_id)
            .display_name(&u16_display_name)
            .hydration_type(HydrationType::Full)
            .population_type(PopulationType::Full)
            .icon(
                U16String::from_str("%SystemRoot%\\system32\\charmap.exe"),
                0,
            )
            .version(u16str!("1.0.0"))
            .recycle_bin_uri(u16str!("http://cloudmirror.example.com/recyclebin"))
            .register(Path::new(&client_path))
            .unwrap();
    }

    mark_in_sync(Path::new(&client_path), &sftp);

    let connection = Session::new()
        .connect(&client_path, Filter { sftp })
        .unwrap();

    wait_for_ctrlc();

    connection.disconnect().unwrap();
    sync_root_id.unregister().unwrap();
}

fn get_client_path() -> String {
    env::var("CLIENT_PATH").unwrap()
}

fn mark_in_sync(path: &Path, sftp: &Sftp) {
    let base = get_client_path();
    for entry in path.read_dir().unwrap() {
        let entry = entry.unwrap();
        let remote_path = entry.path().strip_prefix(&base).unwrap().to_owned();

        let Ok(meta) = sftp.stat(&remote_path) else {
            continue;
        };
        if meta.is_dir() != entry.file_type().unwrap().is_dir() {
            continue;
        }

        let mut options = ConvertOptions::default()
            .mark_in_sync()
            .blob(remote_path.clone().into_os_string().into_encoded_bytes());
        let mut placeholder = match meta.is_dir() {
            true => {
                options = options.has_children();
                Placeholder::open(entry.path()).unwrap()
            }
            false => File::open(entry.path()).unwrap().into(),
        };

        _ = placeholder
            .convert_to_placeholder(options, None)
            .inspect_err(|e| println!("convert_to_placeholder {:?}", e));

        if meta.is_dir() {
            mark_in_sync(&entry.path(), sftp);
        }
    }
}

pub struct Filter {
    sftp: Sftp,
}

impl Filter {
    pub fn remove_remote_dir_all(&self, dest: &Path) -> Result<(), ssh2::Error> {
        for entry in self.sftp.readdir(dest)? {
            match entry.1.is_dir() {
                true => self.remove_remote_dir_all(&entry.0)?,
                false => self.sftp.unlink(&entry.0)?,
            }
        }

        self.sftp.rmdir(dest)
    }
}

impl SyncFilter for Filter {
    fn fetch_data(&self, request: Request, ticket: ticket::FetchData, info: info::FetchData) {
        let path = Path::new(unsafe { OsStr::from_encoded_bytes_unchecked(request.file_blob()) });

        let range = info.required_file_range();
        let end = range.end;
        let mut position = range.start;

        println!(
            "fetch_data {:?} {:?} {}",
            path,
            range,
            info.interrupted_hydration()
        );

        let res = || -> Result<(), _> {
            let mut server_file = self
                .sftp
                .open(path)
                .map_err(|_| CloudErrorKind::InvalidRequest)?;
            server_file
                .seek(SeekFrom::Start(position))
                .map_err(|_| CloudErrorKind::InvalidRequest)?;

            let mut buffer = [0; DOWNLOAD_CHUNK_SIZE_BYTES];

            loop {
                let mut bytes_read = server_file
                    .read(&mut buffer)
                    .map_err(|_| CloudErrorKind::InvalidRequest)?;

                let unaligned = bytes_read % 4096;
                if unaligned != 0 && position + (bytes_read as u64) < end {
                    bytes_read -= unaligned;
                    server_file
                        .seek(SeekFrom::Current(-(unaligned as i64)))
                        .unwrap();
                }
                ticket
                    .write_at(&buffer[0..bytes_read], position)
                    .map_err(|_| CloudErrorKind::InvalidRequest)?;
                position += bytes_read as u64;

                if position >= end {
                    break;
                }

                ticket.report_progress(end, position).unwrap();
            }

            Ok(())
        }();

        if let Err(e) = res {
            ticket.fail(e).unwrap();
        }
    }

    fn deleted(&self, _request: Request, _info: info::Deleted) {
        println!("deleted");
    }

    fn delete(&self, request: Request, ticket: ticket::Delete, info: info::Delete) {
        println!("delete {:?}", request.path());
        let path = Path::new(unsafe { OsStr::from_encoded_bytes_unchecked(request.file_blob()) });
        let res = || -> Result<(), _> {
            match info.is_directory() {
                true => self
                    .remove_remote_dir_all(path)
                    .map_err(|_| CloudErrorKind::InvalidRequest)?,
                false => self
                    .sftp
                    .unlink(path)
                    .map_err(|_| CloudErrorKind::InvalidRequest)?,
            }
            ticket.pass().unwrap();
            Ok(())
        }();

        if let Err(e) = res {
            ticket.fail(e).unwrap();
        }
    }

    fn rename(&self, request: Request, ticket: ticket::Rename, info: info::Rename) {
        let res = || -> Result<(), _> {
            let src = request.path();
            let dest = info.target_path();
            let base = get_client_path();

            println!(
                "rename {} to {}, source in scope: {}, target in scope: {}",
                src.display(),
                dest.display(),
                info.source_in_scope(),
                info.target_in_scope()
            );

            match (info.source_in_scope(), info.target_in_scope()) {
                (true, true) => {
                    self.sftp
                        .rename(
                            src.strip_prefix(&base).unwrap(),
                            dest.strip_prefix(&base).unwrap(),
                            None,
                        )
                        .map_err(|_| CloudErrorKind::InvalidRequest)?;
                }
                (true, false) => {}
                (false, true) => Err(CloudErrorKind::NotSupported)?, // TODO
                (false, false) => Err(CloudErrorKind::InvalidRequest)?,
            }

            ticket.pass().unwrap();
            Ok(())
        }();

        if let Err(e) = res {
            ticket.fail(e).unwrap();
        }
    }

    fn fetch_placeholders(
        &self,
        request: Request,
        ticket: ticket::FetchPlaceholders,
        info: info::FetchPlaceholders,
    ) {
        println!(
            "fetch_placeholders {:?} {:?}",
            request.path(),
            info.pattern()
        );
        let absolute = request.path();
        let client_path = get_client_path();
        let parent = absolute.strip_prefix(&client_path).unwrap();

        let dirs = self.sftp.readdir(parent).unwrap();
        let mut placeholders = dirs
            .into_iter()
            .filter(|(path, _)| !Path::new(&client_path).join(path).exists())
            .map(|(path, stat)| {
                println!("path: {:?}, stat {:?}", path, stat);
                println!("is file: {}, is dir: {}", stat.is_file(), stat.is_dir());

                let relative_path = path.strip_prefix(parent).unwrap();
                PlaceholderFile::new(relative_path)
                    .metadata(
                        match stat.is_dir() {
                            true => Metadata::directory(),
                            false => Metadata::file(),
                        }
                        .size(stat.size.unwrap_or_default())
                        .accessed(
                            stat.atime
                                .and_then(|t| FileTime::from_unix_time(t as _).ok())
                                .unwrap_or_default(),
                        ),
                    )
                    .mark_in_sync()
                    .overwrite()
                    .blob(path.into_os_string().into_encoded_bytes())
            })
            .collect::<Vec<_>>();

        ticket.pass_with_placeholder(&mut placeholders).unwrap();
    }

    fn closed(&self, request: Request, info: info::Closed) {
        println!("closed {:?}, deleted {}", request.path(), info.deleted());
    }

    fn cancel_fetch_data(&self, _request: Request, _info: info::CancelFetchData) {
        println!("cancel_fetch_data");
    }

    fn validate_data(
        &self,
        _request: Request,
        ticket: ticket::ValidateData,
        _info: info::ValidateData,
    ) {
        println!("validate_data");
        #[allow(unused_must_use)]
        {
            ticket.fail(CloudErrorKind::NotSupported);
        }
    }

    fn cancel_fetch_placeholders(&self, _request: Request, _info: info::CancelFetchPlaceholders) {
        println!("cancel_fetch_placeholders");
    }

    fn opened(&self, request: Request, _info: info::Opened) {
        println!("opened: {:?}", request.path());
    }

    fn dehydrate(&self, _request: Request, ticket: ticket::Dehydrate, _info: info::Dehydrate) {
        println!("dehydrate");
        #[allow(unused_must_use)]
        {
            ticket.fail(CloudErrorKind::NotSupported);
        }
    }

    fn dehydrated(&self, _request: Request, _info: info::Dehydrated) {
        println!("dehydrated");
    }

    fn renamed(&self, _request: Request, _info: info::Renamed) {
        println!("renamed");
    }
}

#[derive(Error, Debug)]
pub enum SftpError {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Sftp(#[from] ssh2::Error),
}

fn wait_for_ctrlc() {
    let (tx, rx) = mpsc::channel();

    ctrlc::set_handler(move || {
        tx.send(()).unwrap();
    })
    .expect("Error setting Ctrl-C handler");

    rx.recv().unwrap();
}
