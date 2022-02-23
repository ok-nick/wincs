use std::{
    ffi::OsString,
    mem::ManuallyDrop,
    path::Path,
    sync::{Arc, Weak},
};

use windows::{
    core,
    Win32::{
        Storage::CloudFilters::{self, CfConnectSyncRoot, CF_CONNECT_FLAGS},
        System::{
            Com::{self, CoCreateInstance},
            Search::{self, ISearchCatalogManager, ISearchManager},
        },
    },
};

use crate::{
    filter::{proxy, SyncFilter},
    provider::Provider,
    root::set_flag,
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

    pub fn connect<P, T>(self, path: P, filter: &Arc<T>) -> core::Result<Provider>
    where
        P: AsRef<Path>,
        T: SyncFilter + 'static,
    {
        // TODO: add an option for this and state how it's automatically done if under the user
        index_path(path.as_ref())?;

        let result = unsafe {
            CfConnectSyncRoot(
                path.as_ref().as_os_str(),
                // TODO: ManuallyDrop prevents it from being destructured?
                // Instead store the array in the returned provider
                ManuallyDrop::new(proxy::callbacks::<T>()).as_ptr(),
                // create a weak arc so that it could be upgraded when it's being used and when the
                // original (users) arc is dropped then the program is done
                Weak::into_raw(Arc::downgrade(filter)) as *const _,
                // This is enabled by default to remove the Option requirement around the
                // `path` method from the `Request` struct. To notify the shell of file
                // transfer progress the path is required.
                // TODO: does this mean ^ or does it just mean the path isn't relative to the sync root?
                self.0 | CloudFilters::CF_CONNECT_FLAG_REQUIRE_FULL_FILE_PATH,
            )
        };

        result.map(|key| Provider::new(key.0))
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
