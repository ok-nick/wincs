use std::{
    ffi::OsString,
    path::Path,
    sync::{Arc, Weak},
};

use windows::{
    core,
    Win32::{
        Storage::CloudFilters::{
            self, CfConnectSyncRoot, CF_CALLBACK_REGISTRATION, CF_CONNECT_FLAGS,
        },
        System::{
            Com::{self, CoCreateInstance},
            Search::{self, ISearchCatalogManager, ISearchManager},
        },
    },
};

use crate::{
    filter::{proxy, SyncFilter},
    key::OwnedConnectionKey,
    session::Session,
    utility::set_flag,
};

#[derive(Debug, Clone, Copy)]
pub struct ConnectOptions(CF_CONNECT_FLAGS);

impl ConnectOptions {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn require_process_info(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            CloudFilters::CF_CONNECT_FLAG_REQUIRE_PROCESS_INFO,
            yes,
        );
        self
    }

    // pub fn require_full_file_path(mut self, yes: bool) -> Self {
    //     set_flag(
    //         &mut self.0,
    //         CloudFilters::CF_CONNECT_FLAG_REQUIRE_FULL_FILE_PATH,
    //         yes,
    //     );
    //     self
    // }

    #[must_use]
    pub fn block_self_implicit_hydration(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            CloudFilters::CF_CONNECT_FLAG_BLOCK_SELF_IMPLICIT_HYDRATION,
            yes,
        );
        self
    }

    pub fn connect<P, T>(
        self,
        path: P,
        filter: T,
    ) -> core::Result<Session<([CF_CALLBACK_REGISTRATION; 14], Arc<T>)>>
    where
        P: AsRef<Path>,
        T: SyncFilter + 'static,
    {
        // https://github.com/microsoft/Windows-classic-samples/blob/27ffb0811ca761741502feaefdb591aebf592193/Samples/CloudMirror/CloudMirror/Utilities.cpp#L19
        index_path(path.as_ref())?;

        let filter = Arc::new(filter);
        let callbacks = proxy::callbacks::<T>();
        unsafe {
            CfConnectSyncRoot(
                path.as_ref().as_os_str(),
                // I'm assuming the caller is responsible for freeing this memory and the filter's memory?
                callbacks.as_ptr(),
                // create a weak arc so that it could be upgraded when it's being used and when the
                // connection is closed the filter could be freed
                Weak::into_raw(Arc::downgrade(&filter)) as *const _,
                // This is enabled by default to remove the Option requirement around the
                // `path` method from the `Request` struct
                // TODO: does this mean ^ or does it just mean the path isn't relative to the sync root?
                self.0 | CloudFilters::CF_CONNECT_FLAG_REQUIRE_FULL_FILE_PATH,
            )
        }
        .map(|key| Session::new(OwnedConnectionKey::new(key.0, (callbacks, filter))))
    }
}

impl Default for ConnectOptions {
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

        let catalog: ISearchCatalogManager = searcher.GetCatalog("SystemIndex")?;

        let mut url = OsString::from("file:///");
        url.push(path);

        let crawler = catalog.GetCrawlScopeManager()?;
        crawler.AddDefaultScopeRule(url, true, Search::FF_INDEXCOMPLEXURLS.0 as u32)?;
        crawler.SaveAll()
    }
}
