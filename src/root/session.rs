use std::{
    ffi::OsString,
    fs::OpenOptions,
    mem::{self, MaybeUninit},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Sender, TryRecvError},
        Arc, Weak,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use widestring::{u16cstr, U16CString, U16Str};
use windows::{
    core::{self, PCWSTR},
    Win32::{
        Foundation::{ERROR_IO_INCOMPLETE, HANDLE, WIN32_ERROR},
        Storage::{
            CloudFilters::{self, CfConnectSyncRoot, CF_CONNECT_FLAGS},
            FileSystem::{
                ReadDirectoryChangesW, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED,
                FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_INFORMATION,
            },
        },
        System::{
            Com::{self, CoCreateInstance},
            Search::{self, ISearchManager},
            IO::{CancelIoEx, GetOverlappedResult},
        },
    },
};

use crate::{
    filter::{self, AsyncBridge, Filter, SyncFilter},
    root::connect::Connection,
    utility::LocalBoxFuture,
};

/// A builder to create a new connection for the sync root at the specified path.
#[derive(Debug, Clone, Copy)]
pub struct Session(CF_CONNECT_FLAGS);

impl Session {
    /// Create a new [Session][crate::Session].
    pub fn new() -> Self {
        Self::default()
    }

    /// The [block_implicit_hydration][crate::Session::block_implicit_hydration] flag will prevent
    /// implicit placeholder hydrations from invoking
    /// [SyncFilter::fetch_data][crate::filter::SyncFilter::fetch_data]. This could occur when an
    /// anti-virus is scanning file system activity on files within the sync root.
    ///
    /// A call to the [Placeholder::hydrate][crate::placeholder::Placeholder::hydrate] trait will not be blocked by this flag.
    pub fn block_implicit_hydration(mut self) -> Self {
        self.0 |= CloudFilters::CF_CONNECT_FLAG_BLOCK_SELF_IMPLICIT_HYDRATION;
        self
    }

    /// Initiates a connection to the sync root with the given [SyncFilter].
    pub fn connect<P, F>(self, path: P, filter: F) -> core::Result<Connection<F>>
    where
        P: AsRef<Path>,
        F: SyncFilter + 'static,
    {
        // https://github.com/microsoft/Windows-classic-samples/blob/27ffb0811ca761741502feaefdb591aebf592193/Samples/CloudMirror/CloudMirror/Utilities.cpp#L19
        index_path(path.as_ref())?;

        let filter = Arc::new(filter);
        let callbacks = filter::callbacks::<F>();
        let key = unsafe {
            CfConnectSyncRoot(
                PCWSTR(
                    U16CString::from_os_str(path.as_ref())
                        .expect("not contains nul")
                        .as_ptr(),
                ),
                callbacks.as_ptr(),
                // create a weak arc so that it could be upgraded when it's being used and when the
                // connection is closed, the filter could be freed
                Some(Weak::into_raw(Arc::downgrade(&filter)) as *const _),
                // This is enabled by default to remove the Option requirement around various fields of the
                // [Request][crate::Request] struct
                self.0
                    | CloudFilters::CF_CONNECT_FLAG_REQUIRE_FULL_FILE_PATH
                    | CloudFilters::CF_CONNECT_FLAG_REQUIRE_PROCESS_INFO,
            )
        }?;

        let (cancel_token, join_handle) =
            spawn_root_watcher(path.as_ref().to_path_buf(), filter.clone());

        Ok(Connection::new(
            key.0,
            cancel_token,
            join_handle,
            callbacks,
            filter,
        ))
    }

    /// Initiates a connection to the sync root with the given [Filter].
    pub fn connect_async<P, F, B>(
        self,
        path: P,
        filter: F,
        block_on: B,
    ) -> core::Result<Connection<AsyncBridge<F, B>>>
    where
        P: AsRef<Path>,
        F: Filter + 'static,
        B: Fn(LocalBoxFuture<'_, ()>) + Send + Sync + 'static,
    {
        self.connect(path, AsyncBridge::new(filter, block_on))
    }
}

impl Default for Session {
    fn default() -> Self {
        Self(CloudFilters::CF_CONNECT_FLAG_NONE)
    }
}

fn index_path(path: &Path) -> core::Result<()> {
    unsafe {
        let searcher: ISearchManager = CoCreateInstance(
            &Search::CSearchManager as *const _,
            None,
            Com::CLSCTX_SERVER,
        )?;

        let catalog = searcher.GetCatalog(PCWSTR(u16cstr!("SystemIndex").as_ptr()))?;

        let mut url = OsString::from("file:///");
        url.push(path);

        let crawler = catalog.GetCrawlScopeManager()?;
        crawler.AddDefaultScopeRule(
            PCWSTR(
                U16CString::from_os_str(url)
                    .expect("not contains nul")
                    .as_ptr(),
            ),
            true,
            Search::FF_INDEXCOMPLEXURLS.0 as u32,
        )?;

        crawler.SaveAll()
    }
}

fn spawn_root_watcher<T: SyncFilter + 'static>(
    path: PathBuf,
    filter: Arc<T>,
) -> (Sender<()>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        const CHANGE_BUF_SIZE: usize = 1024;

        let sync_root = OpenOptions::new()
            .access_mode(FILE_LIST_DIRECTORY.0)
            .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED).0)
            .open(&path)
            .expect("sync root directory is opened");
        let mut changes_buf = MaybeUninit::<[u8; CHANGE_BUF_SIZE]>::zeroed();
        let mut overlapped = MaybeUninit::zeroed();
        let mut transferred = MaybeUninit::zeroed();

        while matches!(rx.try_recv(), Err(TryRecvError::Empty)) {
            unsafe {
                ReadDirectoryChangesW(
                    HANDLE(sync_root.as_raw_handle() as _),
                    changes_buf.as_mut_ptr() as *mut _,
                    CHANGE_BUF_SIZE as _,
                    true,
                    FILE_NOTIFY_CHANGE_ATTRIBUTES,
                    None,
                    Some(overlapped.as_mut_ptr()),
                    None,
                )
            }
            .expect("read directory changes");

            loop {
                if let Err(e) = unsafe {
                    GetOverlappedResult(
                        HANDLE(sync_root.as_raw_handle() as _),
                        overlapped.as_ptr(),
                        transferred.as_mut_ptr(),
                        false,
                    )
                } {
                    if e.code() != ERROR_IO_INCOMPLETE.to_hresult() {
                        panic!(
                            "get overlapped result: {:?}, expected: {ERROR_IO_INCOMPLETE:?}",
                            WIN32_ERROR::from_error(&e),
                        );
                    }

                    // cancel by user
                    if !matches!(rx.try_recv(), Err(TryRecvError::Empty)) {
                        _ = unsafe {
                            CancelIoEx(
                                HANDLE(sync_root.as_raw_handle() as _),
                                Some(overlapped.as_ptr()),
                            )
                        };
                        return;
                    }

                    thread::sleep(Duration::from_millis(300));
                    continue;
                }

                if unsafe { transferred.assume_init() } == 0 {
                    break;
                }

                let mut changes = Vec::with_capacity(8);
                let mut entry = changes_buf.as_ptr() as *const FILE_NOTIFY_INFORMATION;
                loop {
                    let relative = unsafe {
                        U16Str::from_ptr(
                            &(*entry).FileName as *const _,
                            (*entry).FileNameLength as usize / mem::size_of::<u16>(),
                        )
                    };

                    changes.push(path.join(relative.to_os_string()));

                    if unsafe { *entry }.NextEntryOffset == 0 {
                        break;
                    }
                    entry = unsafe { entry.byte_add((*entry).NextEntryOffset as _) };
                }

                filter.state_changed(changes);
                break;
            }
        }
    });

    (tx, handle)
}
