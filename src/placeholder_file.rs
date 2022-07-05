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

// TODO: this struct could probably have a better name to represent files/dirs
/// A builder for creating new placeholder files/directories.
#[derive(Debug)]
pub struct PlaceholderFile<'a>(CF_PLACEHOLDER_CREATE_INFO, PhantomData<&'a ()>);

impl<'a> PlaceholderFile<'a> {
    /// Creates a new [PlaceholderFile][crate::PlaceholderFile].
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks this [PlaceholderFile][crate::PlaceholderFile] as having no child placeholders on
    /// creation.
    ///
    /// If [PopulationType::Full][crate::PopulationType] is specified on registration, this flag
    /// will prevent [SyncFilter::fetch_placeholders][crate::SyncFilter::fetch_placeholders] from
    /// being called for this placeholder.
    ///
    /// Only applicable to placeholder directories.
    pub fn has_no_children(mut self) -> Self {
        self.0.Flags |= CloudFilters::CF_PLACEHOLDER_CREATE_FLAG_DISABLE_ON_DEMAND_POPULATION;
        self
    }

    /// Marks the [PlaceholderFile][crate::PlaceholderFile] as synced.
    ///
    /// This flag is used to determine the status of a placeholder shown in the file explorer. It
    /// is applicable to both files and directories.
    ///
    /// A file or directory should be marked as "synced" when it has all of its data and metadata.
    /// A file that is partially full could still be marked as synced, any remaining data will
    /// invoke the [SyncFilter::fetch_data][crate::SyncFilter::fetch_data] callback automatically
    /// if requested.
    pub fn mark_sync(mut self) -> Self {
        self.0.Flags |= CloudFilters::CF_PLACEHOLDER_CREATE_FLAG_MARK_IN_SYNC;
        self
    }

    /// Whether or not to overwrite an existing placeholder.
    pub fn overwrite(mut self) -> Self {
        self.0.Flags |= CloudFilters::CF_PLACEHOLDER_CREATE_FLAG_SUPERSEDE;
        self
    }

    /// Blocks this placeholder file from being dehydrated.
    ///
    /// This flag does not work on directories.
    pub fn block_dehydration(mut self) -> Self {
        self.0.Flags |= CloudFilters::CF_PLACEHOLDER_CREATE_FLAG_ALWAYS_FULL;
        self
    }

    /// The metadata for the [PlaceholderFile][crate::PlaceholderFile].
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.0.FsMetadata = metadata.0;
        self
    }

    /// A buffer of bytes stored with the file that could be accessed through a
    /// [Request::file_blob][crate::Request::file_blob] or
    /// [FileExit::placeholder_info][crate::ext::FileExt::placeholder_info].
    ///
    /// The buffer must not exceed
    /// [4KiB](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Storage/CloudFilters/constant.CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH.html).
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

    /// Creates a placeholder file/directory on the file system.
    ///
    /// The value returned is the final [Usn][crate::Usn] after the placeholder is created.
    ///
    /// It is recommended to use this function over
    /// [FileExt::to_placeholder][crate::ext::FileExt::to_placeholder] for efficiency purposes. If you
    /// need to create multiple placeholders, consider using [BatchCreate][crate::BatchCreate].
    ///
    /// If you need to create placeholders from a callback, do not use this method. Instead, use
    /// [Request::create_placeholder][crate::Request::create_placeholder].
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

/// Creates multiple placeholder file/directories within the given path.
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

/// The metadata for a [PlaceholderFile][crate::PlaceholderFile].
#[derive(Debug, Clone, Copy, Default)]
pub struct Metadata(pub(crate) CF_FS_METADATA);

impl Metadata {
    /// Creates a new [Metadata][crate::Metadata].
    pub fn new() -> Self {
        Self::default()
    }

    /// The time the file/directory was created.
    pub fn creation_time(mut self, time: u64) -> Self {
        self.0.BasicInfo.CreationTime = time as i64;
        self
    }

    /// The time the file/directory was last accessed.
    pub fn last_access_time(mut self, time: u64) -> Self {
        self.0.BasicInfo.LastAccessTime = time as i64;
        self
    }

    /// The time the file/directory content was last written.
    pub fn last_write_time(mut self, time: u64) -> Self {
        self.0.BasicInfo.LastWriteTime = time as i64;
        self
    }

    /// The time the file/directory content or metadata was changed.
    pub fn change_time(mut self, time: u64) -> Self {
        self.0.BasicInfo.ChangeTime = time as i64;
        self
    }

    /// The size of the file's content.
    pub fn size(mut self, size: u64) -> Self {
        self.0.FileSize = size as i64;
        self
    }

    // TODO: create a method for specifying that it's a directory.
    /// File attributes.
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
