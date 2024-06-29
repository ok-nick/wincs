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

/// An API extension to [Path][std::path::Path]
pub trait PathExt
where
    Self: AsRef<Path>,
{
    /// Whether or not the path is located inside of a sync root.
    fn in_sync_root(&self) -> bool {
        self.sync_root_info().is_ok()
    }

    // TODO: This call requires a struct to be made for getters of StorageProviderSyncRootInfo
    /// Information about the sync root that the path is located in.
    fn sync_root_info(&self) -> core::Result<StorageProviderSyncRootInfo> {
        StorageProviderSyncRootManager::GetSyncRootInformationForFolder(
            StorageFolder::GetFolderFromPathAsync(
                &U16String::from_os_str(self.as_ref().as_os_str()).to_hstring(),
            )?
            .get()?,
        )
    }

    // FIXME: This function is not work at all, the CF_PLACEHOLDER_STATE always be 0 or 1
    // fn placeholder_state(&self) -> core::Result<CF_PLACEHOLDER_STATE> {
    //     let path = U16CString::from_os_str(self.as_ref()).unwrap();
    //     let mut file_data = MaybeUninit::zeroed();
    //     unsafe {
    //         FindFirstFileW(PCWSTR(path.as_ptr()), file_data.as_mut_ptr());
    //         Ok(CfGetPlaceholderStateFromFindData(
    //             file_data.assume_init_ref() as *const _ as *const _,
    //         ))
    //     }
    // }
}

impl<T: AsRef<Path>> PathExt for T {}
