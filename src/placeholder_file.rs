use std::{fs, marker::PhantomData, os::windows::prelude::MetadataExt, path::Path, ptr};

use widestring::U16CString;
use windows::{
    core,
    Win32::{
        Foundation,
        Storage::{
            CloudFilters::{
                self, CfCreatePlaceholders, CF_FS_METADATA, CF_PLACEHOLDER_CREATE_INFO,
            },
            FileSystem::FILE_BASIC_INFO,
        },
    },
};

use crate::usn::Usn;

#[derive(Debug)]
pub struct PlaceholderFile<'a>(CF_PLACEHOLDER_CREATE_INFO, PhantomData<&'a ()>);

impl<'a> PlaceholderFile<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn children_present(mut self) -> Self {
        self.0.Flags |= CloudFilters::CF_PLACEHOLDER_CREATE_FLAG_DISABLE_ON_DEMAND_POPULATION;
        self
    }

    #[must_use]
    pub fn mark_sync(mut self) -> Self {
        self.0.Flags |= CloudFilters::CF_PLACEHOLDER_CREATE_FLAG_MARK_IN_SYNC;
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
            blob.len() <= CloudFilters::CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH as usize,
            "blob size must not exceed {} bytes, got {} bytes",
            CloudFilters::CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH,
            blob.len()
        );
        self.0.FileIdentity = blob.as_ptr() as *mut _;
        self.0.FileIdentityLength = blob.len() as u32;

        self
    }

    pub fn create<P: AsRef<Path>>(mut self, path: P) -> core::Result<Usn> {
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

        self.0.Result.ok().map(|_| self.0.CreateUsn as Usn)
    }
}

impl Default for PlaceholderFile<'_> {
    fn default() -> Self {
        Self(
            CF_PLACEHOLDER_CREATE_INFO {
                RelativeFileName: Default::default(),
                FsMetadata: Default::default(),
                FileIdentity: ptr::null_mut(),
                // this is required only for files, who knows why
                FileIdentityLength: 1,
                Flags: CloudFilters::CF_PLACEHOLDER_CREATE_FLAG_NONE,
                Result: Foundation::S_OK,
                CreateUsn: 0,
            },
            PhantomData,
        )
    }
}

pub trait BatchCreate {
    fn create<P: AsRef<Path>>(&mut self, path: P) -> core::Result<Vec<core::Result<Usn>>>;
}

impl BatchCreate for [PlaceholderFile<'_>] {
    fn create<P: AsRef<Path>>(&mut self, path: P) -> core::Result<Vec<core::Result<Usn>>> {
        unsafe {
            CfCreatePlaceholders(
                path.as_ref().as_os_str(),
                self.as_mut_ptr() as *mut CF_PLACEHOLDER_CREATE_INFO,
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
                    .map(|_| placeholder.0.CreateUsn as Usn)
            })
            .collect())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Metadata(pub(crate) CF_FS_METADATA);

impl Metadata {
    pub fn new() -> Self {
        Self::default()
    }

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
    pub fn size(mut self, size: u64) -> Self {
        self.0.FileSize = size as i64;
        self
    }

    #[must_use]
    pub fn attributes(mut self, attributes: u32) -> Self {
        self.0.BasicInfo.FileAttributes = attributes;
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
