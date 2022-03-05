use std::path::Path;

use widestring::U16String;
use windows::{
    core,
    Storage::{
        Provider::{StorageProviderSyncRootInfo, StorageProviderSyncRootManager},
        StorageFolder,
    },
};

use crate::utility::ToHString;

pub trait PathExt
where
    Self: AsRef<Path>,
{
    fn in_sync_root(&self) -> bool {
        self.sync_root_info().is_ok()
    }

    // TODO: This call requires a struct to be made for getters of StorageProviderSyncRootInfo
    fn sync_root_info(&self) -> core::Result<StorageProviderSyncRootInfo> {
        StorageProviderSyncRootManager::GetSyncRootInformationForFolder(
            StorageFolder::GetFolderFromPathAsync(
                &U16String::from_os_str(self.as_ref().as_os_str()).to_hstring(),
            )?
            .get()?,
        )
    }
}

impl<T: AsRef<Path>> PathExt for T {}
