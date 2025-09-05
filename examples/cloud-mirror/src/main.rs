use std::{
    fs::{self, File},
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

use rkyv::{rancor::Error as RkyvError, with::AsString, Archive, Deserialize, Serialize};
use wfd::DialogParams;
use widestring::U16String;
use wincs::{
    info, ticket, HydrationType, PlaceholderFile, PopulationType, Registration, Request,
    SecurityId, Session, SupportedAttributes, SyncFilter, SyncRootIdBuilder,
};

// MUST be a multiple of 4096
const CHUNK_SIZE_BYTES: usize = 4096;
// const CHUNK_DELAY_MS: u64 = 250;
const CHUNK_DELAY_MS: u64 = 0;

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

    let sync_root_id = SyncRootIdBuilder::new(PROVIDER_NAME.into())
        .account_name(ACCOUNT_NAME.into())
        .user_security_id(SecurityId::current_user().unwrap())
        .build()
        .unwrap();

    // Register the sync root
    let display_name = U16String::from_str(DISPLAY_NAME);
    let version = U16String::from_str(VERSION);
    let recycle_uri = U16String::from_str("http://cloudmirror.example.com/recyclebin");

    Registration::from_sync_root_id(&sync_root_id)
        .display_name(display_name.as_ref())
        .icon("%SystemRoot%\\system32\\charmap.exe".into(), 0)
        .version(version.as_ref())
        .recycle_bin_uri(recycle_uri.as_ref())
        .hydration_type(HydrationType::Full)
        .population_type(PopulationType::AlwaysFull)
        .supported_attributes(
            SupportedAttributes::new()
                .file_creation_time()
                .directory_creation_time(),
        )
        .allow_hardlinks()
        .show_siblings_as_group()
        .register(&client_path)
        .unwrap();

    // Connect to the sync root
    let provider = Session::new()
        .connect(
            &client_path,
            Filter {
                client_path: client_path.clone(),
                server_path: server_path.clone(),
            },
        )
        .unwrap();

    create_placeholders(&server_path, &client_path, Path::new(""));

    // TODO: hydrate and dehydrate on pin/unpin

    // wait until a key is pressed to exit
    let (tx, rx) = mpsc::channel();
    ctrlc::set_handler(move || tx.send(()).unwrap()).unwrap();
    rx.recv().unwrap();

    provider.disconnect().unwrap();

    sync_root_id.unregister().unwrap();

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
    #[rkyv(with = AsString)]
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
        let relative_full_path = relative_path.join(&file_name);

        let blob = rkyv::to_bytes::<RkyvError>(&FileBlob {
            relative_path: relative_full_path.clone(),
        })
        .unwrap();

        let placeholder_path = client_path.join(&relative_full_path);

        if !placeholder_path.exists() {
            PlaceholderFile::new(&file_name)
                .metadata(metadata.into())
                .has_no_children()
                .mark_sync()
                .blob(blob.to_vec())
                .create::<&PathBuf>(&client_path.join(relative_path))
                .unwrap();
        }

        if is_dir {
            create_placeholders(server_path, client_path, &relative_full_path);
        }

        // TODO: apply custom state to placeholder like in sample
    }
}

#[derive(Debug)]
struct Filter {
    #[allow(dead_code)]
    client_path: PathBuf,
    server_path: PathBuf,
}

impl SyncFilter for Filter {
    fn fetch_data(&self, request: Request, _ticket: ticket::FetchData, info: info::FetchData) {
        let blob = request.file_blob();
        // convert the blob back to a path
        let archived = rkyv::access::<ArchivedFileBlob, RkyvError>(blob).unwrap();
        let blob: FileBlob = rkyv::deserialize::<FileBlob, RkyvError>(archived).unwrap();

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
    }

    fn validate_data(
        &self,
        request: Request,
        ticket: ticket::ValidateData,
        info: info::ValidateData,
    ) {
        println!("Validating data for {}", request.path().display());
        let range = info.file_range();
        match ticket.pass(range) {
            Ok(()) => println!("Data validated successfully"),
            Err(e) => eprintln!("Error validating data: {:?}", e),
        }
    }

    fn cancel_fetch_data(&self, request: Request, info: info::CancelFetchData) {
        println!("Canceling fetch data for {}", request.path().display());
        if info.timeout() {
            println!("Fetch data timed out");
        } else if info.user_cancelled() {
            println!("Fetch data cancelled by user");
        }
    }

    fn fetch_placeholders(
        &self,
        _request: Request,
        ticket: ticket::FetchPlaceholders,
        _info: info::FetchPlaceholders,
    ) {
        println!("fetch placeholders");
        ticket.pass_with_placeholder(&mut []).unwrap();
    }

    fn cancel_fetch_placeholders(&self, _request: Request, info: info::CancelFetchPlaceholders) {
        println!(
            "Canceling fetch placeholders for {}",
            _request.path().display()
        );
        if info.timeout() {
            println!("Fetch placeholders timed out");
        } else if info.user_cancelled() {
            println!("Fetch placeholders cancelled by user");
        }
    }

    fn opened(&self, request: Request, _info: info::Opened) {
        println!("file opened {:?}", request.path());
    }

    fn closed(&self, request: Request, _info: info::Closed) {
        println!("file closed {:?}", request.path());
    }

    fn dehydrate(&self, _request: Request, _ticket: ticket::Dehydrate, _info: info::Dehydrate) {
        println!("dehydrate");
    }

    fn dehydrated(&self, _request: Request, _info: info::Dehydrated) {
        println!("dehydrated");
    }

    fn delete(&self, _request: Request, _ticket: ticket::Delete, _info: info::Delete) {
        println!("delete");
    }

    fn deleted(&self, _request: Request, _info: info::Deleted) {
        println!("deleted");
    }

    fn rename(&self, request: Request, ticket: ticket::Rename, info: info::Rename) {
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

        _ = ticket.pass();
    }

    fn renamed(&self, _request: Request, _info: info::Renamed) {
        println!("renamed");
    }
}

/*
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
*/
