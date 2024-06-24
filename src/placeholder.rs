use std::{
    fs::File,
    mem,
    ops::Range,
    os::windows::io::{FromRawHandle, IntoRawHandle},
    path::Path,
    ptr,
};

use widestring::U16CString;
use windows::{
    core::{self, PCWSTR},
    Win32::{
        Foundation::{CloseHandle, ERROR_NOT_A_CLOUD_FILE, HANDLE},
        Storage::CloudFilters::{
            self, CfCloseHandle, CfConvertToPlaceholder, CfGetPlaceholderInfo,
            CfOpenFileWithOplock, CfRevertPlaceholder, CfSetInSyncState, CfSetPinState,
            CfUpdatePlaceholder, CF_CONVERT_FLAGS, CF_FILE_RANGE, CF_OPEN_FILE_FLAGS, CF_PIN_STATE,
            CF_PLACEHOLDER_STANDARD_INFO, CF_SET_PIN_FLAGS, CF_UPDATE_FLAGS,
        },
    },
};

use crate::{metadata::Metadata, usn::Usn};

/// The type of handle that the placeholder file/directory owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceholderHandleType {
    /// A handle that was opened with [CfOpenFileWithOplock].
    CfApi,
    /// A handle that was opened with [CreateFile] etc.
    Win32,
}

/// An owned handle to a placeholder file/directory.
///
/// This closes the handle on drop.
#[derive(Debug)]
pub struct OwnedPlaceholderHandle {
    handle_type: PlaceholderHandleType,
    handle: HANDLE,
}

impl OwnedPlaceholderHandle {
    /// Create a new [OwnedPlaceholderHandle] from a handle returned by [CfOpenFileWithOplock].
    ///
    /// # Safety
    ///
    /// The handle must be valid and owned by the caller.
    pub unsafe fn from_cfapi(handle: HANDLE) -> Self {
        Self {
            handle_type: PlaceholderHandleType::CfApi,
            handle,
        }
    }

    /// Create a new [OwnedPlaceholderHandle] from a handle returned by [CreateFile] etc.
    ///
    /// # Safety
    ///
    /// The handle must be valid and owned by the caller.
    pub unsafe fn from_win32(handle: HANDLE) -> Self {
        Self {
            handle_type: PlaceholderHandleType::Win32,
            handle,
        }
    }

    pub const fn handle(&self) -> HANDLE {
        self.handle
    }

    pub const fn handle_type(&self) -> PlaceholderHandleType {
        self.handle_type
    }
}

impl Drop for OwnedPlaceholderHandle {
    fn drop(&mut self) {
        match self.handle_type {
            PlaceholderHandleType::CfApi => unsafe { CfCloseHandle(self.handle) },
            PlaceholderHandleType::Win32 => unsafe {
                CloseHandle(self.handle);
            },
        }
    }
}

/// Options for opening a placeholder file/directory.
pub struct OpenOptions {
    flags: CF_OPEN_FILE_FLAGS,
}

impl OpenOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn exclusive(mut self) -> Self {
        self.flags |= CloudFilters::CF_OPEN_FILE_FLAG_EXCLUSIVE;
        self
    }

    pub fn write_access(mut self) -> Self {
        self.flags |= CloudFilters::CF_OPEN_FILE_FLAG_WRITE_ACCESS;
        self
    }

    pub fn delete_access(mut self) -> Self {
        self.flags |= CloudFilters::CF_OPEN_FILE_FLAG_DELETE_ACCESS;
        self
    }

    pub fn foreground(mut self) -> Self {
        self.flags |= CloudFilters::CF_OPEN_FILE_FLAG_FOREGROUND;
        self
    }

    /// Open the placeholder file/directory using `CfOpenFileWithOplock`.
    pub fn open(self, path: impl AsRef<Path>) -> core::Result<Placeholder> {
        let u16_path = U16CString::from_os_str(path.as_ref()).unwrap();
        let handle = unsafe { CfOpenFileWithOplock(PCWSTR(u16_path.as_ptr()), self.flags) }?;
        Ok(Placeholder {
            handle: unsafe { OwnedPlaceholderHandle::from_cfapi(handle) },
        })
    }
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self {
            flags: CloudFilters::CF_OPEN_FILE_FLAG_NONE,
        }
    }
}

/// The pin state of a placeholder.
///
/// [Read more
/// here](https://docs.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_pin_state#remarks)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinState {
    /// The platform could decide freely.
    Unspecified,
    /// [SyncFilter::fetch_data][crate::SyncFilter::fetch_data] will be called to hydrate the rest
    /// of the placeholder's data. Any dehydration requests will fail automatically.
    Pinned,
    /// [SyncFilter::dehydrate][crate::SyncFilter::dehydrate] will be called to dehydrate the rest
    /// of the placeholder's data.
    Unpinned,
    /// The placeholder will never sync to the cloud.
    Excluded,
    /// The placeholder will inherit the parent placeholder's pin state.
    Inherit,
}

impl From<PinState> for CF_PIN_STATE {
    fn from(state: PinState) -> Self {
        match state {
            PinState::Unspecified => CloudFilters::CF_PIN_STATE_UNSPECIFIED,
            PinState::Pinned => CloudFilters::CF_PIN_STATE_PINNED,
            PinState::Unpinned => CloudFilters::CF_PIN_STATE_UNPINNED,
            PinState::Excluded => CloudFilters::CF_PIN_STATE_EXCLUDED,
            PinState::Inherit => CloudFilters::CF_PIN_STATE_INHERIT,
        }
    }
}

impl From<CF_PIN_STATE> for PinState {
    fn from(state: CF_PIN_STATE) -> Self {
        match state {
            CloudFilters::CF_PIN_STATE_UNSPECIFIED => PinState::Unspecified,
            CloudFilters::CF_PIN_STATE_PINNED => PinState::Pinned,
            CloudFilters::CF_PIN_STATE_UNPINNED => PinState::Unpinned,
            CloudFilters::CF_PIN_STATE_EXCLUDED => PinState::Excluded,
            CloudFilters::CF_PIN_STATE_INHERIT => PinState::Inherit,
            _ => unreachable!(),
        }
    }
}

/// The placeholder pin flags.
#[derive(Debug, Clone, Copy)]
pub struct PinOptions(CF_SET_PIN_FLAGS);

impl PinOptions {
    /// Applies the pin state to all descendants of the placeholder (if the placeholder is a
    /// directory).
    pub fn recurse(&mut self) -> &mut Self {
        self.0 |= CloudFilters::CF_SET_PIN_FLAG_RECURSE;
        self
    }

    /// Applies the pin state to all descendants of the placeholder excluding the current one (if
    /// the placeholder is a directory).
    pub fn recurse_children(&mut self) -> &mut Self {
        self.0 |= CloudFilters::CF_SET_PIN_FLAG_RECURSE_ONLY;
        self
    }

    /// Stop applying the pin state when the first error is encountered. Otherwise, skip over it
    /// and keep applying.
    pub fn stop_on_error(&mut self) -> &mut Self {
        self.0 |= CloudFilters::CF_SET_PIN_FLAG_RECURSE_STOP_ON_ERROR;
        self
    }
}

impl Default for PinOptions {
    fn default() -> Self {
        Self(CloudFilters::CF_SET_PIN_FLAG_NONE)
    }
}

/// File to placeholder file conversion parameters.
#[derive(Debug, Clone)]
pub struct ConvertOptions {
    flags: CF_CONVERT_FLAGS,
    blob: Vec<u8>,
}

impl ConvertOptions {
    /// Marks a placeholder as in sync.
    ///
    /// See also
    /// [SetInSyncState](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfsetinsyncstate),
    /// [What does "In-Sync" Mean?](https://www.userfilesystem.com/programming/faq/#nav_whatdoesin-syncmean)
    pub fn mark_in_sync(mut self) -> Self {
        self.flags |= CloudFilters::CF_CONVERT_FLAG_MARK_IN_SYNC;
        self
    }

    /// Dehydrate the placeholder after conversion.
    ///
    /// This flag is only applicable to files.
    pub fn dehydrate(mut self) -> Self {
        self.flags |= CloudFilters::CF_CONVERT_FLAG_DEHYDRATE;
        self
    }

    // TODO: make the name of this function more specific
    /// Marks the placeholder as "partially full," such that [SyncFilter::fetch_placeholders][crate::SyncFilter::fetch_placeholders]
    /// will be invoked when this directory is next accessed so that the remaining placeholders are inserted.
    ///
    /// Only applicable to placeholder directories.
    pub fn has_children(mut self) -> Self {
        self.flags |= CloudFilters::CF_CONVERT_FLAG_ENABLE_ON_DEMAND_POPULATION;
        self
    }

    /// Blocks this placeholder from being dehydrated.
    ///
    /// This flag does not work on directories.
    pub fn block_dehydration(mut self) -> Self {
        self.flags |= CloudFilters::CF_CONVERT_FLAG_ALWAYS_FULL;
        self
    }

    /// Forces the conversion of a non-cloud placeholder file to a cloud placeholder file.
    ///
    /// Placeholder files are built into the NTFS file system and thus, a placeholder not associated
    /// with the sync root is possible.
    pub fn force(mut self) -> Self {
        self.flags |= CloudFilters::CF_CONVERT_FLAG_FORCE_CONVERT_TO_CLOUD_FILE;
        self
    }

    /// A buffer of bytes stored with the file that could be accessed through a
    /// [Request::file_blob][crate::Request::file_blob] or
    /// [FileExit::placeholder_info][crate::ext::FileExt::placeholder_info].
    ///
    /// The buffer must not exceed
    /// [4KiB](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Storage/CloudFilters/constant.CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH.html).
    pub fn blob(mut self, blob: Vec<u8>) -> Self {
        assert!(
            blob.len() <= CloudFilters::CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH as usize,
            "blob size must not exceed {} bytes, got {} bytes",
            CloudFilters::CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH,
            blob.len()
        );
        self.blob = blob;
        self
    }
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            flags: CloudFilters::CF_CONVERT_FLAG_NONE,
            blob: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct PlaceholderInfo {
    data: Vec<u8>,
    info: *const CF_PLACEHOLDER_STANDARD_INFO,
}

impl PlaceholderInfo {
    pub fn on_disk_data_size(&self) -> i64 {
        unsafe { &*self.info }.OnDiskDataSize
    }

    pub fn validated_data_size(&self) -> i64 {
        unsafe { &*self.info }.ValidatedDataSize
    }
    pub fn modified_data_size(&self) -> i64 {
        unsafe { &*self.info }.ModifiedDataSize
    }
    pub fn properties_size(&self) -> i64 {
        unsafe { &*self.info }.PropertiesSize
    }

    pub fn pin_state(&self) -> PinState {
        unsafe { &*self.info }.PinState.into()
    }

    pub fn is_in_sync(&self) -> bool {
        unsafe { &*self.info }.InSyncState == CloudFilters::CF_IN_SYNC_STATE_IN_SYNC
    }

    pub fn file_id(&self) -> i64 {
        unsafe { &*self.info }.FileId
    }

    pub fn sync_root_file_id(&self) -> i64 {
        unsafe { &*self.info }.SyncRootFileId
    }

    pub fn blob(&self) -> &[u8] {
        &self.data[mem::size_of::<CF_PLACEHOLDER_STANDARD_INFO>()..]
    }
}

impl std::fmt::Debug for PlaceholderInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlaceholderInfo")
            .field("on_disk_data_size", &self.on_disk_data_size())
            .field("validated_data_size", &self.validated_data_size())
            .field("modified_data_size", &self.modified_data_size())
            .field("properties_size", &self.properties_size())
            .field("pin_state", &self.pin_state())
            .field("is_in_sync", &self.is_in_sync())
            .field("file_id", &self.file_id())
            .field("sync_root_file_id", &self.sync_root_file_id())
            .finish()
    }
}

/// Placeholder update parameters.
#[derive(Debug, Clone)]
pub struct UpdateOptions<'a> {
    metadata: Option<Metadata>,
    dehydrate_ranges: Vec<CF_FILE_RANGE>,
    flags: CF_UPDATE_FLAGS,
    blob: &'a [u8],
}

impl<'a> UpdateOptions<'a> {
    /// [Metadata][crate::Metadata] contains file system metadata about the placeholder to be updated.
    ///
    /// File size will be truncates to 0 if not specified, otherwise to the specified size.
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.flags &= !(CloudFilters::CF_UPDATE_FLAG_PASSTHROUGH_FS_METADATA);
        self.metadata = Some(metadata);
        self
    }

    /// Fields in [Metadata][crate::Metadata] will be updated.
    pub fn metadata_all(mut self, metadata: Metadata) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_PASSTHROUGH_FS_METADATA;
        self.metadata = Some(metadata);
        self
    }

    /// Extended ranges to be dehydrated.
    ///
    /// All the offsets and lengths should be `PAGE_SIZE` aligned.
    /// Passing a single range with Offset `0` and Length `CF_EOF` will invalidate the entire file.
    /// This has the same effect as passing the flag `CF_UPDATE_FLAG_DEHYDRATE` instead
    pub fn dehydrate_ranges(mut self, ranges: impl IntoIterator<Item = Range<u64>>) -> Self {
        self.dehydrate_ranges
            .extend(ranges.into_iter().map(|r| CF_FILE_RANGE {
                StartingOffset: r.start as _,
                Length: (r.end - r.start) as _,
            }));
        self
    }

    /// The update will fail if the `IN_SYNC` attribute is not currently set on the placeholder.
    pub fn update_if_in_sync(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_VERIFY_IN_SYNC;
        self
    }

    /// Marks a placeholder as in sync.
    ///
    /// See also
    /// [SetInSyncState](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfsetinsyncstate),
    /// [What does "In-Sync" Mean?](https://www.userfilesystem.com/programming/faq/#nav_whatdoesin-syncmean)
    pub fn mark_in_sync(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_MARK_IN_SYNC;
        self
    }

    /// Marks a placeholder as not in sync. `Sync Pending` will be shown in explorer.
    ///
    /// See also
    /// [SetInSyncState](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfsetinsyncstate),
    /// [What does "In-Sync" Mean?](https://www.userfilesystem.com/programming/faq/#nav_whatdoesin-syncmean)
    pub fn mark_not_in_sync(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_CLEAR_IN_SYNC;
        self
    }

    /// The platform dehydrates the file after updating the placeholder successfully.
    pub fn dehydrate(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_DEHYDRATE;
        self
    }

    /// Disables on-demand population for directories.
    pub fn has_no_children(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_DISABLE_ON_DEMAND_POPULATION;
        self
    }

    /// Enable on-demand population for directories.
    pub fn has_children(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_ENABLE_ON_DEMAND_POPULATION;
        self
    }

    /// Remove the file identity from the placeholder.
    /// [UpdateOptions::blob()](crate::placeholder::UpdateOptions::blob) will be ignored.
    pub fn remove_blob(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_REMOVE_FILE_IDENTITY;
        self
    }

    /// The platform removes all existing extrinsic properties on the placeholder.
    pub fn remove_properties(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_REMOVE_PROPERTY;
        self
    }

    pub fn blob(mut self, blob: &'a [u8]) -> Self {
        assert!(
            blob.len() <= CloudFilters::CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH as usize,
            "blob size must not exceed {} bytes, got {} bytes",
            CloudFilters::CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH,
            blob.len()
        );
        self.blob = blob;
        self
    }
}

impl Default for UpdateOptions<'_> {
    fn default() -> Self {
        Self {
            metadata: None,
            dehydrate_ranges: Vec::new(),
            flags: CloudFilters::CF_UPDATE_FLAG_NONE,
            blob: &[],
        }
    }
}

/// A struct to perform various operations on a placeholder(or regular) file/directory.
#[derive(Debug)]
pub struct Placeholder {
    handle: OwnedPlaceholderHandle,
}

impl Placeholder {
    /// Open options for opening [Placeholder][crate::Placeholder]s.
    pub fn options() -> OpenOptions {
        OpenOptions::default()
    }

    /// Open the placeholder file/directory with `CF_OPEN_FILE_FLAG_NONE`.
    pub fn open(path: impl AsRef<Path>) -> core::Result<Self> {
        OpenOptions::new().open(path)
    }

    /// Marks a placeholder as in sync or not.
    ///
    /// If the passed [USN][crate::Usn] is outdated, the call will fail,
    /// otherwise the [USN][crate::Usn] will be updated.
    ///
    /// See also
    /// [SetInSyncState](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfsetinsyncstate),
    /// [What does "In-Sync" Mean?](https://www.userfilesystem.com/programming/faq/#nav_whatdoesin-syncmean)
    pub fn mark_in_sync<'a>(
        &mut self,
        in_sync: bool,
        usn: impl Into<Option<&'a mut Usn>>,
    ) -> core::Result<&mut Self> {
        unsafe {
            CfSetInSyncState(
                self.handle.handle,
                match in_sync {
                    true => CloudFilters::CF_IN_SYNC_STATE_IN_SYNC,
                    false => CloudFilters::CF_IN_SYNC_STATE_NOT_IN_SYNC,
                },
                CloudFilters::CF_SET_IN_SYNC_FLAG_NONE,
                usn.into()
                    .map_or(ptr::null_mut(), |x| ptr::read(x) as *mut _),
            )
        }?;

        Ok(self)
    }

    /// Sets the pin state of the placeholder.
    ///
    /// See also
    /// [CfSetPinState](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfsetpinstate),
    /// [What does "Pinned" Mean?](https://www.userfilesystem.com/programming/faq/#nav_howdoesthealwayskeeponthisdevicemenuworks)
    pub fn mark_pin(&mut self, state: PinState, options: PinOptions) -> core::Result<&mut Self> {
        unsafe { CfSetPinState(self.handle.handle, state.into(), options.0, ptr::null_mut()) }?;
        Ok(self)
    }

    /// Converts a file to a placeholder file.
    ///
    /// If the passed [USN][crate::Usn] is outdated, the call will fail,
    /// otherwise the [USN][crate::Usn] will be updated.
    ///
    /// See also [CfConvertToPlaceholder](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfconverttoplaceholder).
    pub fn convert_to_placeholder<'a>(
        &mut self,
        options: ConvertOptions,
        usn: impl Into<Option<&'a mut Usn>>,
    ) -> core::Result<&mut Self> {
        unsafe {
            CfConvertToPlaceholder(
                self.handle.handle,
                match options.blob.is_empty() {
                    true => ptr::null(),
                    false => options.blob.as_ptr() as *const _,
                },
                options.blob.len() as _,
                options.flags,
                usn.into()
                    .map_or(ptr::null_mut(), |x| ptr::read(x) as *mut _),
                ptr::null_mut(),
            )
        }?;

        Ok(self)
    }

    /// Gets various characteristics of the placeholder.
    ///
    /// If the `blob_size` not matches the actual size of the blob,
    /// the call will returns `HRESULT_FROM_WIN32(ERROR_MORE_DATA)`.
    /// Returns `None` if the handle not points to a placeholder.
    pub fn info(&self, blob_size: usize) -> core::Result<Option<PlaceholderInfo>> {
        let mut data = vec![0; mem::size_of::<CF_PLACEHOLDER_STANDARD_INFO>() + blob_size];

        let r = unsafe {
            CfGetPlaceholderInfo(
                self.handle.handle,
                CloudFilters::CF_PLACEHOLDER_INFO_STANDARD,
                data.as_mut_ptr() as *mut _,
                data.len() as u32,
                ptr::null_mut(),
            )
        };

        match r {
            Ok(()) => Ok(Some(PlaceholderInfo {
                info: &unsafe {
                    data[..=mem::size_of::<CF_PLACEHOLDER_STANDARD_INFO>()]
                        .align_to::<CF_PLACEHOLDER_STANDARD_INFO>()
                }
                .1[0] as *const _,
                data,
            })),
            Err(e) if e.win32_error() == Some(ERROR_NOT_A_CLOUD_FILE) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Updates various characteristics of a placeholder.
    ///
    /// See also [CfUpdatePlaceholder](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfupdateplaceholder).
    pub fn update<'a>(
        &mut self,
        options: UpdateOptions,
        usn: impl Into<Option<&'a mut Usn>>,
    ) -> core::Result<&mut Self> {
        unsafe {
            CfUpdatePlaceholder(
                self.handle.handle,
                options.metadata.map_or(ptr::null(), |x| &x.0 as *const _),
                match options.blob.is_empty() {
                    true => ptr::null(),
                    false => options.blob.as_ptr() as *const _,
                },
                options.blob.len() as _,
                match options.dehydrate_ranges.is_empty() {
                    true => ptr::null(),
                    false => options.dehydrate_ranges.as_ptr(),
                },
                options.dehydrate_ranges.len() as u32,
                options.flags,
                usn.into()
                    .map_or(ptr::null_mut(), |x| ptr::read(x) as *mut _),
                ptr::null_mut(),
            )
        }?;

        Ok(self)
    }
}

impl From<File> for Placeholder {
    fn from(file: File) -> Self {
        Self {
            handle: unsafe {
                OwnedPlaceholderHandle::from_win32(HANDLE(file.into_raw_handle() as _))
            },
        }
    }
}

impl TryFrom<Placeholder> for File {
    type Error = core::Error;

    #[allow(clippy::missing_transmute_annotations)]
    fn try_from(placeholder: Placeholder) -> core::Result<Self> {
        match placeholder.handle.handle_type {
            PlaceholderHandleType::Win32 => {
                let file =
                    unsafe { File::from_raw_handle(mem::transmute(placeholder.handle.handle)) };
                Ok(file)
            }
            PlaceholderHandleType::CfApi => unsafe {
                CfRevertPlaceholder(
                    placeholder.handle.handle,
                    CloudFilters::CF_REVERT_FLAG_NONE,
                    ptr::null_mut(),
                )
            }
            .map(|_| unsafe { File::from_raw_handle(mem::transmute(placeholder.handle.handle)) }),
        }
    }
}
