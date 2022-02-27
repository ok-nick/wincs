use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    net::TcpStream,
    path::Path,
};

use rkyv::{Archive, Deserialize, Serialize};
use ssh2::{Session, Sftp};
use thiserror::Error;
use wincs::{
    filter::{info, ticket, SyncFilter},
    logger::ErrorReason,
    placeholder_file::{Metadata, PlaceholderFile},
    request::Request,
};

// max should be 65536, this is done both in term-scp and sshfs because it's the
// max packet size for a tcp connection
const DOWNLOAD_CHUNK_SIZE_BYTES: usize = 4096;
// doesn't have to be 4KiB aligned
const UPLOAD_CHUNK_SIZE_BYTES: usize = 4096;

const CLIENT_PATH: &str = "C:\\Users\\nicky\\Music\\sftp_client";

const PROVIDER_NAME: &str = "wincs";
const DISPLAY_NAME: &str = "Sftp";

#[derive(Debug, Archive, Serialize, Deserialize)]
pub struct FileBlob {
    relative_path: String,
}

fn main() {
    let tcp = TcpStream::connect("localhost").unwrap();
    let mut session = Session::new().unwrap();
    session.set_blocking(true);
    session.set_tcp_stream(tcp);
    session.handshake().unwrap();
    session.userauth_agent("nicky").unwrap();

    let sftp = session.sftp().unwrap();

    // do I need the "."?
    for entry in sftp.readdir(Path::new(".")).unwrap() {
        // check if it's a file or dir and set metadata/other stuff accordingly
        PlaceholderFile::new()
            .metadata(
                Metadata::default()
                    // .creation_time() // either the access time or write time, whichever is less
                    .last_access_time(entry.1.atime.unwrap_or_default())
                    .last_write_time(entry.1.mtime.unwrap_or_default())
                    .change_time(entry.1.mtime.unwrap_or_default())
                    .file_size(entry.1.size.unwrap_or_default()),
            )
            .disable_on_demand_population(true)
            .mark_in_sync(true) // need this?
            .blob::<_, 100>(FileBlob {
                relative_path: entry.0.as_os_str().to_owned().to_string_lossy().to_string(),
            })
            .unwrap() // when moved to a recursive function change this
            .create(Path::new(CLIENT_PATH).join(entry.0.file_name().unwrap()))
            .unwrap();
    }

    // TODO: Periodically check for changes on the server and check pin state
}

pub struct Filter {
    sftp: Sftp,
}

impl Filter {
    pub fn create_file(&self, src: &Path, dest: &Path) -> Result<(), SftpError> {
        let mut client_file = File::open(src)?;
        // TODO: This will overwrite the file if it exists on the server
        let mut server_file = self.sftp.create(dest)?;

        let mut buffer = [0; UPLOAD_CHUNK_SIZE_BYTES];
        let mut bytes_written = 0;

        // TODO: I could do the little offset trick and moving the old bytes to the
        // beginning of the buffer, I just don't know if it's worth it
        loop {
            client_file.seek(SeekFrom::Start(bytes_written))?;
            match client_file.read(&mut buffer) {
                Ok(0) => break,
                Ok(bytes_read) => {
                    bytes_written += server_file.write(&buffer[0..bytes_read])? as u64;
                }
                Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
                Err(err) => return Err(SftpError::Io(err)),
            }
        }

        Ok(())
    }

    // TODO: src is full, dest is relative
    pub fn create_dir_all(&self, src: &Path, dest: &Path) -> Result<(), SftpError> {
        // TODO: what does the "o" mean in 0o775
        self.sftp.mkdir(dest, 0o775)?;

        for entry in fs::read_dir(src)? {
            let src = entry?.path();
            let dest = dest.join(src.file_name().unwrap());
            match src.is_dir() {
                true => self.create_dir_all(&src, &dest)?,
                false => self.create_file(&src, &dest)?,
            }
        }

        Ok(())
    }

    pub fn remove_dir_all(&self, dest: &Path) -> Result<(), ssh2::Error> {
        for entry in self.sftp.readdir(dest)? {
            match entry.0.is_dir() {
                true => self.remove_dir_all(&entry.0)?,
                false => self.sftp.unlink(&entry.0)?,
            }
        }

        self.sftp.rmdir(dest)
    }
}

// TODO: handle unwraps
// TODO: everything is just forwarded to external functions... This should be
// changed in the wrapper api
impl SyncFilter for Filter {
    type Error = SftpError;

    // TODO: handle unwraps
    fn fetch_data(&self, request: Request, info: info::FetchData) -> Result<(), Self::Error> {
        // TODO: handle unwrap
        let path =
            Path::new(unsafe { &request.file_blob::<FileBlob, 100>().unwrap().relative_path });

        let range = info.required_file_range();
        let end = range.end;
        let mut position = range.start;

        let mut server_file = self.sftp.open(path)?;
        let mut client_file = BufWriter::with_capacity(4096, request.placeholder());

        server_file.seek(SeekFrom::Start(position))?;
        client_file.seek(SeekFrom::Start(position))?;

        let mut buffer = [0; DOWNLOAD_CHUNK_SIZE_BYTES];

        // TODO: move to a func and remove unwraps & allow to split up the entire read
        // into segments done on separate threads
        // transfer the data in chunks
        loop {
            client_file.get_ref().set_progress(end, position).unwrap();

            // TODO: read directly to the BufWriters buffer
            // TODO: ignore if the error was just interrupted
            let bytes_read = server_file.read(&mut buffer[0..DOWNLOAD_CHUNK_SIZE_BYTES])?;
            let bytes_written = client_file.write(&buffer[0..bytes_read])?;
            position += bytes_written as u64;

            if position >= end {
                break;
            }
        }

        client_file.flush()?;

        Ok(())
    }

    // TODO: I probably also have to delete the file from the disk
    fn delete(
        &self,
        request: Request,
        ticket: ticket::Delete,
        info: info::Delete,
    ) -> Result<(), Self::Error> {
        let path = Path::new(unsafe { request.file_blob::<OsString>() });
        match info.is_directory() {
            true => self.remove_dir_all(path)?,
            false => self.sftp.unlink(path)?,
        }

        Ok(())
    }

    // TODO: Do I have to move the file and set the file progress? or does the OS
    // handle that? (I think I do)
    fn rename(
        &self,
        request: Request,
        ticket: ticket::Rename,
        info: info::Rename,
    ) -> Result<(), Self::Error> {
        match info.target_in_scope() {
            true => {
                // TODO: path should auto include the drive letter
                let src = request.path();
                // TODO: should be relative
                let dest = info.target_path();

                match info.source_in_scope() {
                    // TODO: use fs::copy or fs::rename, whatever it is to move the local files,
                    // then use CovertToPlaceholder. I'm not sure if I have to do this recursively
                    // for each file or only the top-level folder TODO: which
                    // rename flags do I use? how do I know if I should be overwriting?
                    true => self.sftp.rename(&src, &dest, None)?,
                    false => match info.is_directory() {
                        true => self.create_dir_all(&src, &dest)?,
                        false => self.create_file(&src, &dest)?,
                    },
                }
            }
            // TODO: do I need to delete it locally?
            false => self
                .sftp
                .unlink(Path::new(unsafe { request.file_blob::<OsString>() }))?,
        }

        Ok(())
    }

    fn closed(&self, request: Request, info: info::Closed) -> Result<(), Self::Error> {
        Ok(())
    }

    // TODO: acknowledgement callbacks
}

#[derive(Error, Debug)]
pub enum SftpError {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Sftp(#[from] ssh2::Error),
}

impl ErrorReason for SftpError {
    fn code(&self) -> u32 {
        0
    }

    fn message(&self) -> &widestring::U16Str {
        match self {
            SftpError::Io(_) => todo!(),
            SftpError::Sftp(_) => todo!(),
        }
    }

    fn title(&self) -> &widestring::U16Str {
        todo!()
    }
}
