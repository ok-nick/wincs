use std::{fs, os::windows::prelude::MetadataExt, path::Path, ptr};

use widestring::U16CString;
use windows::{
    core,
    Win32::{
        Foundation,
        Storage::{
            CloudFilters::{
                self, CfCreatePlaceholders, CF_FS_METADATA, CF_PLACEHOLDER_CREATE_INFO,
            },
            FileSystem::{self, FILE_BASIC_INFO},
        },
    },
};

use crate::root::set_flag;

#[derive(Debug, Clone)]
pub struct PlaceholderFile(CF_PLACEHOLDER_CREATE_INFO);

// TODO: impl From<File> for PlaceholderFile
impl<'a> PlaceholderFile {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn disable_on_demand_population(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0.Flags,
            CloudFilters::CF_PLACEHOLDER_CREATE_FLAG_DISABLE_ON_DEMAND_POPULATION,
            yes,
        );
        self
    }

    #[must_use]
    pub fn mark_in_sync(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0.Flags,
            CloudFilters::CF_PLACEHOLDER_CREATE_FLAG_MARK_IN_SYNC,
            yes,
        );
        self
    }

    #[must_use]
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.0.FsMetadata = metadata.0;
        self
    }

    #[must_use]
    pub fn blob(mut self, blob: &'a [u8]) -> Self {
        assert!(
            blob.len() <= 4096,
            "blob size must not exceed 4KB (4096 bytes) after serialization, got {} bytes",
            blob.len()
        );
        self.0.FileIdentity = blob.as_ptr() as *mut _;
        self.0.FileIdentityLength = blob.len() as u32;

        self
    }

    pub fn create<P: AsRef<Path>>(mut self, path: P) -> core::Result<u64> {
        let path = path.as_ref();

        // TODO: handle unwraps
        let mut file_name = U16CString::from_os_str(path.file_name().unwrap()).unwrap();
        self.0.RelativeFileName.0 = unsafe { file_name.as_mut_ptr() };

        unsafe {
            CfCreatePlaceholders(
                // TODO: handle unwrap
                path.parent().unwrap().as_os_str(),
                &mut self as *mut _ as *mut _,
                1,
                CloudFilters::CF_CREATE_FLAG_NONE,
                ptr::null_mut(),
            )?;
        }

        self.0.Result.ok().map(|_| self.0.CreateUsn as u64)
    }
}

impl Default for PlaceholderFile {
    fn default() -> Self {
        Self(CF_PLACEHOLDER_CREATE_INFO {
            RelativeFileName: Default::default(),
            FsMetadata: Default::default(),
            // TODO: This one-byte array is only required for files, who knows why
            // How is the array not dropped in this situation?
            FileIdentity: [0u8; 1].as_mut_ptr() as *mut _,
            FileIdentityLength: 1,
            Flags: CloudFilters::CF_PLACEHOLDER_CREATE_FLAG_NONE,
            Result: Foundation::S_OK,
            CreateUsn: 0,
        })
    }
}

pub trait BatchCreate {
    fn create<P: AsRef<Path>>(&self, path: P) -> core::Result<Vec<core::Result<u64>>>;
}

impl BatchCreate for [PlaceholderFile] {
    fn create<P: AsRef<Path>>(&self, path: P) -> core::Result<Vec<core::Result<u64>>> {
        unsafe {
            CfCreatePlaceholders(
                path.as_ref().as_os_str(),
                // TODO: I could avoid an allocation here if I'm able to not store the U16CString
                // in the PlaceholderFile struct
                self.iter()
                    .map(|placeholder| placeholder.0)
                    .collect::<Vec<CF_PLACEHOLDER_CREATE_INFO>>()
                    .as_mut_ptr(),
                self.len() as u32,
                CloudFilters::CF_CREATE_FLAG_NONE,
                ptr::null_mut(),
            )?;
        }

        Ok(self
            .iter()
            .map(|placeholder| {
                placeholder
                    .0
                    .Result
                    .ok()
                    .map(|_| placeholder.0.CreateUsn as u64)
            })
            .collect())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Metadata(pub(crate) CF_FS_METADATA);

impl Metadata {
    #[must_use]
    pub fn creation_time(mut self, time: u64) -> Self {
        self.0.BasicInfo.CreationTime = time as i64;
        self
    }

    #[must_use]
    pub fn last_access_time(mut self, time: u64) -> Self {
        self.0.BasicInfo.LastAccessTime = time as i64;
        self
    }

    #[must_use]
    pub fn last_write_time(mut self, time: u64) -> Self {
        self.0.BasicInfo.LastWriteTime = time as i64;
        self
    }

    #[must_use]
    pub fn change_time(mut self, time: u64) -> Self {
        self.0.BasicInfo.ChangeTime = time as i64;
        self
    }

    #[must_use]
    pub fn file_size(mut self, size: u64) -> Self {
        self.0.FileSize = size as i64;
        self
    }

    #[must_use]
    pub fn readonly(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0.BasicInfo.FileAttributes,
            FileSystem::FILE_ATTRIBUTE_READONLY.0,
            yes,
        );
        self
    }

    // TODO: do file attributes
    #[must_use]
    pub fn file_attributes(mut self, flags: u32) -> Self {
        self.0.BasicInfo.FileAttributes = flags;
        self
    }
}

impl From<fs::Metadata> for Metadata {
    fn from(metadata: fs::Metadata) -> Self {
        Self(CF_FS_METADATA {
            BasicInfo: FILE_BASIC_INFO {
                CreationTime: metadata.creation_time() as i64,
                LastAccessTime: metadata.last_access_time() as i64,
                LastWriteTime: metadata.last_write_time() as i64,
                ChangeTime: metadata.last_write_time() as i64,
                FileAttributes: metadata.file_attributes(),
            },
            FileSize: metadata.file_size() as i64,
        })
    }
}
