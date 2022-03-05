use std::{
    ffi::OsString,
    fs::{self, File},
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

use rkyv::{with::AsString, Archive, Deserialize, Serialize};
use wfd::DialogParams;
use wincs::{
    filter::{info, ticket, SyncFilter},
    logger::ErrorReason,
    placeholder_file::PlaceholderFile,
    request::Request,
    root::{
        connect::ConnectOptions,
        register::{HydrationType, PopulationType, RegisterOptions, SupportedAttributes},
        SyncRoot,
    },
};

// MUST be a multiple of 4096
const CHUNK_SIZE_BYTES: usize = 4096;
// const CHUNK_DELAY_MS: u64 = 250;
const CHUNK_DELAY_MS: u64 = 0;

const SCRATCH_SPACE: usize = 100;

const SERVER_PATH: Option<&str> = Some("C:\\Users\\nicky\\Music\\server");
const CLIENT_PATH: Option<&str> = Some("C:\\Users\\nicky\\Music\\client");

const PROVIDER_NAME: &str = "TestStorageProvider";
const ACCOUNT_NAME: &str = "TestAccount1";
const DISPLAY_NAME: &str = "TestStorageProviderDisplayName";
const VERSION: &str = "1.0.0";

fn main() {
    let server_path = SERVER_PATH
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .unwrap_or_else(|| {
            wfd::open_dialog(DialogParams {
                file_name_label: "Server Folder",
                title: "Select Server Directory",
                options: wfd::FOS_PICKFOLDERS,
                ..Default::default()
            })
            .unwrap()
            .selected_file_path
        });

    let client_path = CLIENT_PATH
        .map(PathBuf::from)
        .filter(|path| path.exists())
        .unwrap_or_else(|| {
            wfd::open_dialog(DialogParams {
                file_name_label: "Client Folder",
                title: "Select Client Directory",
                options: wfd::FOS_PICKFOLDERS,
                ..Default::default()
            })
            .unwrap()
            .selected_file_path
        });

    let sync_root = SyncRoot::new(PROVIDER_NAME.into(), ACCOUNT_NAME.into());

    // impl COM objects

    sync_root
        .register(
            &client_path,
            RegisterOptions::new()
                .display_name(DISPLAY_NAME.into())
                .icon_path("%SystemRoot%\\system32\\charmap.exe,0".into())
                .version(VERSION.into())
                .recycle_bin_uri("http://cloudmirror.example.com/recyclebin".into())
                .hydration_type(HydrationType::Full)
                .population_type(PopulationType::AlwaysFull)
                .supported_attributes(
                    SupportedAttributes::new()
                        .file_creation_time(true)
                        .directory_creation_time(true),
                )
                .allow_hardlinks(false)
                .show_siblings_as_group(false),
        )
        .unwrap();

    let provider = ConnectOptions::new()
        .require_process_info(true)
        .connect(
            &client_path,
            &Arc::new(Filter {
                client_path: client_path.clone(),
                server_path: server_path.clone(),
            }),
        )
        .unwrap();

    create_placeholders(&server_path, &client_path, Path::new(""));

    // TODO: hydrate and dehydrate on pin/unpin

    // wait until a key is pressed to exit
    let (tx, rx) = mpsc::channel();
    ctrlc::set_handler(move || tx.send(()).unwrap()).unwrap();
    rx.recv().unwrap();

    provider.disconnect().unwrap();

    sync_root.unregister().unwrap();

    // cleanup any placeholders whilst keeping the client folder intact
    fs::read_dir(&client_path).unwrap().for_each(|entry| {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            fs::remove_dir_all(entry.path()).unwrap()
        } else {
            fs::remove_file(entry.path()).unwrap()
        }
    });
}

#[derive(Debug, Archive, Serialize, Deserialize)]
struct FileBlob {
    #[with(AsString)]
    relative_path: PathBuf,
}

fn create_placeholders(server_path: &Path, client_path: &Path, relative_path: &Path) {
    for entry in fs::read_dir(server_path.join(relative_path))
        .unwrap()
        .flatten()
    {
        let metadata = entry.metadata().unwrap();
        let is_dir = metadata.is_dir();

        let file_name = entry.file_name();
        let relative_path = relative_path.join(&file_name);

        rkyv::to_bytes::<_, 100>(&FileBlob {
            relative_path: relative_path.clone(),
        });

        let placeholder_path = client_path.join(&relative_path);
        if !placeholder_path.exists() {
            PlaceholderFile::new()
                .metadata(metadata.into())
                .disable_on_demand_population(true)
                .mark_in_sync(true)
                .blob::<_, SCRATCH_SPACE>(FileBlob {
                    relative_path: relative_path.clone(),
                })
                .unwrap()
                .create(&placeholder_path)
                .unwrap();
        }

        if is_dir {
            create_placeholders(server_path, client_path, &relative_path);
        }

        // TODO: apply custom state to placeholder like in sample
    }
}

#[derive(Debug)]
struct Filter {
    client_path: PathBuf,
    server_path: PathBuf,
}

impl SyncFilter for Filter {
    type Error = FilterError;

    fn fetch_data(&self, request: Request, info: info::FetchData) -> Result<(), Self::Error> {
        let blob = request.file_blob::<FileBlob, SCRATCH_SPACE>().unwrap();

        // TODO: this is the same as just using path with the drive letter attached
        // + 1 is to account for the path separator
        let mut server_path = PathBuf::with_capacity(
            self.server_path.as_os_str().len() + blob.relative_path.as_os_str().len() + 1,
        );
        server_path.push(&self.server_path);
        server_path.push(blob.relative_path);

        // due to `PopulationPolicy::AlwaysFull`, this will always be the range of the
        // entire file (I think)
        let range = info.required_file_range();
        let end = range.end;
        let mut position = range.start;

        // buffered capacity of 4KiB to comply with the windows api
        // TODO: if > 4096 bytes are read and not eof then this will error, need to use aligned_writer
        let mut client_file = BufWriter::with_capacity(4096, request.placeholder());
        let mut server_file = File::open(server_path).unwrap();

        server_file.seek(SeekFrom::Start(position)).unwrap();
        client_file.seek(SeekFrom::Start(position)).unwrap();

        // reuse the buffer to avoid allocations
        let mut buffer = [0; CHUNK_SIZE_BYTES];

        // TODO: if anything in here fails then just keep retrying like in the sample
        // TODO: create a less naive impl
        loop {
            // set the progress (transfer dialog + progress bar) in the beginning of the
            // loop to account for 0 progress and to make it seem more responsive
            client_file.get_ref().set_progress(end, position).unwrap();

            // TODO: read directly to the BufWriters buffer
            // TODO: ignore interrupted errors
            let bytes_read = server_file.read(&mut buffer).unwrap();
            let bytes_written = client_file.write(&buffer[0..bytes_read]).unwrap();
            position += bytes_written as u64;

            // if everything is downloaded then we're done
            if position >= end {
                break;
            }

            // simulate network latency
            thread::sleep(Duration::from_millis(CHUNK_DELAY_MS))
        }

        // ensure any remaining data is written
        client_file.flush().unwrap();

        // TODO: if anything fails (remove unwraps) then call TransferData with
        // a failure CompletionStatus

        Ok(())
    }

    fn validate_data(
        &self,
        request: Request,
        ticket: ticket::ValidateData,
        info: info::ValidateData,
    ) -> Result<(), Self::Error> {
        println!("validate data");
        Ok(())
    }

    fn cancel_fetch_data(&self, request: Request, info: info::Cancel) -> Result<(), Self::Error> {
        println!("cancel fetch data");
        Ok(())
    }

    fn fetch_placeholders(
        &self,
        request: Request,
        info: info::FetchPlaceholders,
    ) -> Result<(), Self::Error> {
        println!("fetch placeholders");
        Ok(())
    }

    fn cancel_fetch_placeholders(
        &self,
        request: Request,
        info: info::Cancel,
    ) -> Result<(), Self::Error> {
        println!("cancel fetch placeholders");
        Ok(())
    }

    fn opened(&self, request: Request, info: info::Opened) -> Result<(), Self::Error> {
        println!("file opened {:?}", request.path());
        Ok(())
    }

    fn closed(&self, request: Request, info: info::Closed) -> Result<(), Self::Error> {
        println!("file closed {:?}", request.path());
        Ok(())
    }

    fn dehydrate(
        &self,
        request: Request,
        ticket: ticket::Dehydrate,
        info: info::Dehydrate,
    ) -> Result<(), Self::Error> {
        println!("dehydrate");
        Ok(())
    }

    fn dehydrated(&self, request: Request, info: info::Dehydrated) -> Result<(), Self::Error> {
        println!("dehydrated");

        Ok(())
    }

    fn delete(
        &self,
        request: Request,
        ticket: ticket::Delete,
        info: info::Delete,
    ) -> Result<(), Self::Error> {
        println!("delete");
        Ok(())
    }

    fn deleted(&self, request: Request, info: info::Deleted) -> Result<(), Self::Error> {
        println!("deleted");
        Ok(())
    }

    fn rename(
        &self,
        request: Request,
        ticket: ticket::Rename,
        info: info::Rename,
    ) -> Result<(), Self::Error> {
        let source_path = request.path();
        println!(
            "rename\n\tsource_path: {:?}\n\ttarget_path: {:?}",
            source_path,
            info.target_path()
        );

        match info.target_in_scope() {
            true => match info.source_in_scope() {
                true => {
                    println!(
                        "move file/directory within sync root, {:?}",
                        info.target_path()
                    );
                }
                false => match info.is_directory() {
                    true => {
                        println!("move directory into sync root");
                    }
                    false => {
                        println!("move file into sync root");
                    }
                },
            },
            false => match info.is_directory() {
                true => {
                    println!("move directory outside sync root");
                }
                false => {
                    println!("move file outside sync root");
                }
            },
        }

        Ok(())
    }

    fn renamed(&self, request: Request, info: info::Renamed) -> Result<(), Self::Error> {
        println!("renamed");
        Ok(())
    }
}

pub struct FilterError;

impl ErrorReason for FilterError {
    fn code(&self) -> u32 {
        0
    }

    fn message(&self) -> &widestring::U16Str {
        todo!()
    }

    fn title(&self) -> &widestring::U16Str {
        todo!()
    }
}
