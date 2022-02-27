use std::path::{Path, PathBuf};

use widestring::U16String;
use windows::{
    core,
    Storage::{
        Provider::{StorageProviderSyncRootInfo, StorageProviderSyncRootManager},
        StorageFolder,
    },
};

use crate::utility::hstring_from_widestring;

pub trait PathExt {
    // TODO: if `sync_root_info` doesn't error then this is true
    fn in_sync_root(&self) -> bool {
        todo!()
    }

    // TODO: uses `info_from_path`. This call requires a struct to be made for getters of StorageProviderSyncRootInfo
    fn sync_root_info(&self) {
        todo!()
    }
}

impl PathExt for PathBuf {}
impl PathExt for Path {}

pub fn info_from_path(path: &Path) -> core::Result<StorageProviderSyncRootInfo> {
    StorageProviderSyncRootManager::GetSyncRootInformationForFolder(
        StorageFolder::GetFolderFromPathAsync(hstring_from_widestring(&U16String::from_os_str(
            path.as_os_str(),
        )))?
        .get()?,
    )
}
