use std::{
    fs::File,
    mem::{self, MaybeUninit},
    ops::{Bound, Range, RangeBounds},
    os::windows::{io::AsRawHandle, prelude::RawHandle},
    ptr,
};

use widestring::U16CStr;
use windows::{
    core,
    Win32::{
        Foundation::HANDLE,
        Storage::{
            CloudFilters::{
                self, CfConvertToPlaceholder, CfDehydratePlaceholder, CfGetPlaceholderInfo,
                CfGetPlaceholderRangeInfo, CfGetPlaceholderStateFromFileInfo,
                CfGetSyncRootInfoByHandle, CfHydratePlaceholder, CfRevertPlaceholder,
                CfSetInSyncState, CfSetPinState, CfUpdatePlaceholder, CF_CONVERT_FLAGS,
                CF_FILE_RANGE, CF_PIN_STATE, CF_PLACEHOLDER_RANGE_INFO_CLASS,
                CF_PLACEHOLDER_STANDARD_INFO, CF_PLACEHOLDER_STATE, CF_SET_PIN_FLAGS,
                CF_SYNC_PROVIDER_STATUS, CF_SYNC_ROOT_INFO_STANDARD, CF_SYNC_ROOT_STANDARD_INFO,
                CF_UPDATE_FLAGS,
            },
            FileSystem::{self, GetFileInformationByHandleEx, FILE_ATTRIBUTE_TAG_INFO},
        },
    },
};

use crate::{
    placeholder_file::Metadata,
    root::{HydrationPolicy, HydrationType, PopulationType, SupportedAttributes},
    usn::Usn,
};

/// An API extension to [File][std::fs::File].
pub trait FileExt: AsRawHandle {
    /// Converts a file to a placeholder file, returning the resulting USN.
    ///
    /// Restrictions:
    /// * The file or directory must be the sync root directory itself, or a descendant directory.
    ///     * [CloudErrorKind::NotUnderSyncRoot][crate::CloudErrorKind::NotUnderSyncRoot]
    /// * If [ConvertOptions::dehydrate][ConvertOptions::dehydrate] is selected, the sync root must
    /// not be registered with [HydrationType::AlwaysFull][crate::HydrationType::AlwaysFull].
    ///     * [CloudErrorKind::NotSupported][crate::CloudErrorKind::NotSupported]
    /// * If [ConvertOptions::dehydrate][ConvertOptions::dehydrate] is selected, the placeholder
    /// must not be pinned.
    ///     * [CloudErrorKind::Pinned][crate::CloudErrorKind::Pinned]
    /// * The handle must have write access.
    ///     * [CloudErrorKind::AccessDenied][crate::CloudErrorKind::AccessDenied]
    ///
    /// [Read more
    /// here](https://docs.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfconverttoplaceholder#remarks]
    // TODO: the 4th remark on the link doesn't make sense? Seems to be copied and pasted from
    // `CfUpdatePlaceholder`.
    fn to_placeholder(&self, options: ConvertOptions) -> core::Result<Usn> {
        let mut usn = MaybeUninit::<i64>::uninit();
        unsafe {
            CfConvertToPlaceholder(
                HANDLE(self.as_raw_handle() as isize),
                options
                    .blob
                    .map_or(ptr::null(), |blob| blob.as_ptr() as *const _),
                options.blob.map_or(0, |blob| blob.len() as u32),
                options.flags,
                usn.as_mut_ptr(),
                ptr::null_mut(),
            )
            .map(|_| usn.assume_init() as Usn)
        }
    }

    /// Converts a placeholder file to a normal file.
    ///
    /// Restrictions:
    /// * If the file is not already hydrated, it will implicitly call
    /// [SyncFilter::fetch_data][crate::SyncFilter::fetch_data]. If the file can not be hydrated,
    /// the conversion will fail.
    /// The handle must have write access.
    fn to_file(&self) -> core::Result<()> {
        unsafe {
            CfRevertPlaceholder(
                HANDLE(self.as_raw_handle() as isize),
                CloudFilters::CF_REVERT_FLAG_NONE,
                ptr::null_mut(),
            )
        }
    }

    /// Updates various characteristics of a placeholder.
    ///
    /// Restrictions:
    /// * The file or directory must be the sync root directory itself, or a descendant directory.
    ///     * [CloudErrorKind::NotUnderSyncRoot][crate::CloudErrorKind::NotUnderSyncRoot]
    /// * If [UpdateOptions::dehydrate][UpdateOptions::dehydrate] is selected, the sync root must
    /// not be registered with [HydrationType::AlwaysFull][crate::HydrationType::AlwaysFull].
    ///     * [CloudErrorKind::NotSupported][crate::CloudErrorKind::NotSupported]
    /// * If [UpdateOptions::dehydrate][UpdateOptions::dehydrate] is selected, the placeholder
    /// must not be pinned.
    ///     * [CloudErrorKind::Pinned][crate::CloudErrorKind::Pinned]
    /// * If [UpdateOptions::dehydrate][UpdateOptions::dehydrate] is selected, the placeholder
    /// must be in sync.
    ///     * [CloudErrorKind::NotInSync][crate::CloudErrorKind::NotInSync]
    /// * The handle must have write access.
    ///     * [CloudErrorKind::AccessDenied][crate::CloudErrorKind::AccessDenied]
    // TODO: this could be split into multiple functions to make common patterns easier
    fn update(&self, usn: Usn, mut options: UpdateOptions) -> core::Result<Usn> {
        let mut usn = usn as i64;
        unsafe {
            CfUpdatePlaceholder(
                HANDLE(self.as_raw_handle() as isize),
                options.metadata.map_or(ptr::null(), |x| &x.0 as *const _),
                options.blob.map_or(ptr::null(), |x| x.as_ptr() as *const _),
                options.blob.map_or(0, |x| x.len() as u32),
                options.dehydrate_range.as_mut_ptr(),
                options.dehydrate_range.len() as u32,
                options.flags,
                &mut usn as *mut _,
                ptr::null_mut(),
            )
            .map(|_| usn as Usn)
        }
    }

    /// Hydrates a placeholder file.
    // TODO: doc restrictions. I believe the remarks are wrong in that this call requires both read
    // and write access? https://docs.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfhydrateplaceholder#remarks
    fn hydrate<T: RangeBounds<u64>>(&self, range: T) -> core::Result<()> {
        unsafe {
            CfHydratePlaceholder(
                HANDLE(self.as_raw_handle() as isize),
                match range.start_bound() {
                    Bound::Included(x) => *x as i64,
                    Bound::Excluded(x) => x.saturating_add(1) as i64,
                    Bound::Unbounded => 0,
                },
                match range.end_bound() {
                    Bound::Included(x) => *x as i64,
                    Bound::Excluded(x) => x.saturating_sub(1) as i64,
                    Bound::Unbounded => -1,
                },
                CloudFilters::CF_HYDRATE_FLAG_NONE,
                ptr::null_mut(),
            )
        }
    }

    /// Dehydrates a placeholder file.
    fn dehydrate<T: RangeBounds<u64>>(&self, range: T) -> core::Result<()> {
        dehydrate(self.as_raw_handle(), range, false)
    }

    /// Dehydrates a placeholder file as a system process running in the background. Otherwise, it
    /// is called on behalf of a logged-in user.
    fn background_dehydrate<T: RangeBounds<u64>>(&self, range: T) -> core::Result<()> {
        dehydrate(self.as_raw_handle(), range, true)
    }

    /// Reads raw data in a placeholder file without invoking the [SyncFilter][crate::SyncFilter].
    fn read_raw(&self, read_type: ReadType, offset: u64, buffer: &mut [u8]) -> core::Result<u32> {
        // TODO: buffer length must be u32 max
        let mut length = 0;
        unsafe {
            CfGetPlaceholderRangeInfo(
                HANDLE(self.as_raw_handle() as isize),
                read_type.into(),
                offset as i64,
                buffer.len() as i64,
                buffer as *mut _ as *mut _,
                buffer.len() as u32,
                &mut length as *mut _,
            )
        }
        .map(|_| length)
    }

    /// Gets various characteristics of a placeholder.
    fn placeholder_info(&self) -> core::Result<PlaceholderInfo> {
        // TODO: same as below except finds the size after 2 calls of CfGetPlaceholderInfo
        todo!()
    }

    /// Gets various characteristics of a placeholder using the passed blob size.
    fn placeholder_info_unchecked(&self, blob_size: usize) -> core::Result<PlaceholderInfo> {
        let mut data = vec![0; mem::size_of::<CF_PLACEHOLDER_STANDARD_INFO>() + blob_size];

        unsafe {
            CfGetPlaceholderInfo(
                HANDLE(self.as_raw_handle() as isize),
                CloudFilters::CF_PLACEHOLDER_INFO_STANDARD,
                data.as_mut_ptr() as *mut _,
                data.len() as u32,
                ptr::null_mut(),
            )?;
        }

        Ok(PlaceholderInfo {
            info: &unsafe {
                data[..=mem::size_of::<CF_PLACEHOLDER_STANDARD_INFO>()]
                    .align_to::<CF_PLACEHOLDER_STANDARD_INFO>()
            }
            .1[0] as *const _,
            data,
        })
    }

    /// Gets the current state of the placeholder.
    // TODO: test to ensure this works. I feel like returning an option here is a little odd in the
    // case of a non parsable state.
    fn placeholder_state(&self) -> core::Result<Option<PlaceholderState>> {
        let mut info = MaybeUninit::<FILE_ATTRIBUTE_TAG_INFO>::zeroed();
        unsafe {
            GetFileInformationByHandleEx(
                HANDLE(self.as_raw_handle() as isize),
                FileSystem::FileAttributeTagInfo,
                info.as_mut_ptr() as *mut _,
                mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
            )
            .ok()?;

            PlaceholderState::try_from_win32(CfGetPlaceholderStateFromFileInfo(
                &info.assume_init() as *const _ as *const _,
                FileSystem::FileAttributeTagInfo,
            ))
        }
    }

    /// Sets the pin state of the placeholder.
    fn set_pin_state(&self, state: PinState, options: PinOptions) -> core::Result<()> {
        unsafe {
            CfSetPinState(
                HANDLE(self.as_raw_handle() as isize),
                state.into(),
                options.0,
                ptr::null_mut(),
            )
        }
    }

    /// Marks a placeholder as synced.
    ///
    /// If the passed [USN][crate::Usn] is outdated, the call will fail.
    // TODO: must have write access
    fn mark_sync(&self, usn: Usn) -> core::Result<Usn> {
        mark_sync_state(self.as_raw_handle(), true, usn)
    }

    /// Marks a placeholder as not in sync.
    ///
    /// If the passed [USN][crate::Usn] is outdated, the call will fail.
    // TODO: must have write access
    fn mark_unsync(&self, usn: Usn) -> core::Result<Usn> {
        mark_sync_state(self.as_raw_handle(), false, usn)
    }

    /// Returns whether or not the handle is a valid placeholder.
    fn is_placeholder(&self) -> core::Result<bool> {
        self.placeholder_state().map(|state| state.is_some())
    }

    /// Gets various characteristics of the sync root.
    fn sync_root_info(&self) -> core::Result<SyncRootInfo> {
        // TODO: this except finds the size after 2 calls of CfGetSyncRootInfoByHandle
        todo!()
    }

    /// Gets various characteristics of a placeholder using the passed blob size.
    fn sync_root_info_unchecked(&self, blob_size: usize) -> core::Result<SyncRootInfo> {
        let mut data = vec![0; mem::size_of::<CF_SYNC_ROOT_STANDARD_INFO>() + blob_size];

        unsafe {
            CfGetSyncRootInfoByHandle(
                HANDLE(self.as_raw_handle() as isize),
                CF_SYNC_ROOT_INFO_STANDARD,
                data.as_mut_ptr() as *mut _,
                data.len() as u32,
                ptr::null_mut(),
            )?;
        }

        Ok(SyncRootInfo {
            info: &unsafe {
                data[..=mem::size_of::<CF_SYNC_ROOT_STANDARD_INFO>()]
                    .align_to::<CF_SYNC_ROOT_STANDARD_INFO>()
            }
            .1[0] as *const _,
            data,
        })
    }

    /// Returns whether or not the handle is inside of a sync root.
    fn in_sync_root() -> core::Result<bool> {
        // TODO: this should use the uwp apis
        todo!()
    }
}

fn mark_sync_state(handle: RawHandle, sync: bool, usn: Usn) -> core::Result<Usn> {
    // TODO: docs say the usn NEEDS to be a null pointer? Why? Is it not supported?
    // https://docs.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfsetinsyncstate
    let mut usn = usn as i64;
    unsafe {
        CfSetInSyncState(
            HANDLE(handle as isize),
            if sync {
                CloudFilters::CF_IN_SYNC_STATE_IN_SYNC
            } else {
                CloudFilters::CF_IN_SYNC_STATE_NOT_IN_SYNC
            },
            CloudFilters::CF_SET_IN_SYNC_FLAG_NONE,
            &mut usn as *mut _,
        )
        .map(|_| usn as u64)
    }
}

// TODO: is `CfDehydratePlaceholder` deprecated?
// https://docs.microsoft.com/en-us/answers/questions/723805/what-is-the-behavior-of-file-ranges-in-different-p.html
fn dehydrate<T: RangeBounds<u64>>(
    handle: RawHandle,
    range: T,
    background: bool,
) -> core::Result<()> {
    unsafe {
        CfDehydratePlaceholder(
            HANDLE(handle as isize),
            match range.start_bound() {
                Bound::Included(x) => *x as i64,
                Bound::Excluded(x) => x.saturating_add(1) as i64,
                Bound::Unbounded => 0,
            },
            match range.end_bound() {
                Bound::Included(x) => *x as i64,
                Bound::Excluded(x) => x.saturating_sub(1) as i64,
                // This behavior is documented in CfDehydratePlaceholder
                Bound::Unbounded => -1,
            },
            if background {
                CloudFilters::CF_DEHYDRATE_FLAG_NONE
            } else {
                CloudFilters::CF_DEHYDRATE_FLAG_BACKGROUND
            },
            ptr::null_mut(),
        )
    }
}

impl FileExt for File {}

/// The type of data to read from a placeholder.
#[derive(Debug, Copy, Clone)]
pub enum ReadType {
    /// Any data that is saved to the disk.
    Saved,
    /// Data that has been synced to the cloud.
    Validated,
    /// Data that has not synced to the cloud.
    Modified,
}

impl From<ReadType> for CF_PLACEHOLDER_RANGE_INFO_CLASS {
    fn from(read_type: ReadType) -> Self {
        match read_type {
            ReadType::Saved => CloudFilters::CF_PLACEHOLDER_RANGE_INFO_ONDISK,
            ReadType::Validated => CloudFilters::CF_PLACEHOLDER_RANGE_INFO_VALIDATED,
            ReadType::Modified => CloudFilters::CF_PLACEHOLDER_RANGE_INFO_MODIFIED,
        }
    }
}

/// Information about a sync root.
#[derive(Debug)]
pub struct SyncRootInfo {
    data: Vec<u8>,
    info: *const CF_SYNC_ROOT_STANDARD_INFO,
}

// TODO: most of the returns only have setters, no getters
impl SyncRootInfo {
    /// The file ID of the sync root.
    pub fn file_id(&self) -> u64 {
        unsafe { &*self.info }.SyncRootFileId as u64
    }

    /// The hydration policy of the sync root.
    pub fn hydration_policy(&self) -> HydrationType {
        unsafe { &*self.info }.HydrationPolicy.Primary.into()
    }

    /// The hydration type of the sync root.
    pub fn hydration_type(&self) -> HydrationPolicy {
        unsafe { &*self.info }.HydrationPolicy.Modifier.into()
    }

    /// The population type of the sync root.
    pub fn population_type(&self) -> PopulationType {
        unsafe { &*self.info }.PopulationPolicy.Primary.into()
    }

    /// The attributes supported by the sync root.
    pub fn supported_attributes(&self) -> SupportedAttributes {
        unsafe { &*self.info }.InSyncPolicy.into()
    }

    /// Whether or not hardlinks are allowed by the sync root.
    pub fn hardlinks_allowed(&self) -> bool {
        unsafe { &*self.info }.HardLinkPolicy == CloudFilters::CF_HARDLINK_POLICY_ALLOWED
    }

    /// The status of the sync provider.
    pub fn status(&self) -> ProviderStatus {
        unsafe { &*self.info }.ProviderStatus.into()
    }

    /// The name of the sync provider.
    pub fn provider_name(&self) -> &U16CStr {
        U16CStr::from_slice_truncate(unsafe { &*self.info }.ProviderName.as_slice()).unwrap()
    }

    /// The version of the sync provider.
    pub fn version(&self) -> &U16CStr {
        U16CStr::from_slice_truncate(unsafe { &*self.info }.ProviderVersion.as_slice()).unwrap()
    }

    /// The register blob associated with the sync root.
    pub fn blob(&self) -> &[u8] {
        &self.data[(mem::size_of::<CF_SYNC_ROOT_STANDARD_INFO>() + 1)..]
    }
}

/// Sync provider status.
#[derive(Debug, Clone, Copy)]
pub enum ProviderStatus {
    /// The sync provider is disconnected.
    Disconnected,
    /// The sync provider is idle.
    Idle,
    /// The sync provider is populating a namespace.
    PopulateNamespace,
    /// The sync provider is populating placeholder metadata.
    PopulateMetadata,
    /// The sync provider is incrementally syncing placeholder content.
    PopulateContent,
    /// The sync provider is incrementally syncing placeholder content.
    SyncIncremental,
    /// The sync provider has fully synced placeholder data.
    SyncFull,
    /// The sync provider has lost connectivity.
    ConnectivityLost,
    // TODO: if setting the sync status is added.
    // ClearFlags,
    /// The sync provider has been terminated.
    Terminated,
    /// The sync provider had an error.
    Error,
}

impl From<CF_SYNC_PROVIDER_STATUS> for ProviderStatus {
    fn from(status: CF_SYNC_PROVIDER_STATUS) -> Self {
        match status {
            CloudFilters::CF_PROVIDER_STATUS_DISCONNECTED => Self::Disconnected,
            CloudFilters::CF_PROVIDER_STATUS_IDLE => Self::Idle,
            CloudFilters::CF_PROVIDER_STATUS_POPULATE_NAMESPACE => Self::PopulateNamespace,
            CloudFilters::CF_PROVIDER_STATUS_POPULATE_METADATA => Self::PopulateContent,
            CloudFilters::CF_PROVIDER_STATUS_POPULATE_CONTENT => Self::PopulateContent,
            CloudFilters::CF_PROVIDER_STATUS_SYNC_INCREMENTAL => Self::SyncIncremental,
            CloudFilters::CF_PROVIDER_STATUS_SYNC_FULL => Self::SyncFull,
            CloudFilters::CF_PROVIDER_STATUS_CONNECTIVITY_LOST => Self::ConnectivityLost,
            // CloudFilters::CF_PROVIDER_STATUS_CLEAR_FLAGS => Self::ClearFlags,
            CloudFilters::CF_PROVIDER_STATUS_TERMINATED => Self::Terminated,
            CloudFilters::CF_PROVIDER_STATUS_ERROR => Self::Error,
            _ => unreachable!(),
        }
    }
}

impl From<ProviderStatus> for CF_SYNC_PROVIDER_STATUS {
    fn from(status: ProviderStatus) -> Self {
        match status {
            ProviderStatus::Disconnected => CloudFilters::CF_PROVIDER_STATUS_DISCONNECTED,
            ProviderStatus::Idle => CloudFilters::CF_PROVIDER_STATUS_IDLE,
            ProviderStatus::PopulateNamespace => {
                CloudFilters::CF_PROVIDER_STATUS_POPULATE_NAMESPACE
            }
            ProviderStatus::PopulateMetadata => CloudFilters::CF_PROVIDER_STATUS_POPULATE_METADATA,
            ProviderStatus::PopulateContent => CloudFilters::CF_PROVIDER_STATUS_POPULATE_CONTENT,
            ProviderStatus::SyncIncremental => CloudFilters::CF_PROVIDER_STATUS_SYNC_INCREMENTAL,
            ProviderStatus::SyncFull => CloudFilters::CF_PROVIDER_STATUS_SYNC_FULL,
            ProviderStatus::ConnectivityLost => CloudFilters::CF_PROVIDER_STATUS_CONNECTIVITY_LOST,
            // ProviderStatus::ClearFlags => CloudFilters::CF_PROVIDER_STATUS_CLEAR_FLAGS,
            ProviderStatus::Terminated => CloudFilters::CF_PROVIDER_STATUS_TERMINATED,
            ProviderStatus::Error => CloudFilters::CF_PROVIDER_STATUS_ERROR,
        }
    }
}

/// The pin state of a placeholder.
///
/// [Read more
/// here](https://docs.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_pin_state#remarks)
#[derive(Debug, Clone, Copy)]
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
    pub fn pin_descendants(&mut self) -> &mut Self {
        self.0 |= CloudFilters::CF_SET_PIN_FLAG_RECURSE;
        self
    }

    /// Applies the pin state to all descendants of the placeholder excluding the current one (if
    /// the placeholder is a directory).
    pub fn pin_descendants_not_self(&mut self) -> &mut Self {
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
pub struct ConvertOptions<'a> {
    flags: CF_CONVERT_FLAGS,
    blob: Option<&'a [u8]>,
}

impl<'a> ConvertOptions<'a> {
    /// Marks the placeholder as synced.
    ///
    /// This flag is used to determine the status of a placeholder shown in the file explorer. It
    /// is applicable to both files and directories.
    ///
    /// A file or directory should be marked as "synced" when it has all of its data and metadata.
    /// A file that is partially full could still be marked as synced, any remaining data will
    /// invoke the [SyncFilter::fetch_data][crate::SyncFilter::fetch_data] callback automatically
    /// if requested.
    pub fn mark_sync(mut self) -> Self {
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

    /// Marks the placeholder as having no child placeholders on creation.
    ///
    /// If [PopulationType::Full][crate::PopulationType] is specified on registration, this flag
    /// will prevent [SyncFilter::fetch_placeholders][crate::SyncFilter::fetch_placeholders] from
    /// being called for this placeholder.
    ///
    /// Only applicable to placeholder directories.
    pub fn has_no_children(mut self) -> Self {
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
    pub fn blob(mut self, blob: &'a [u8]) -> Self {
        assert!(
            blob.len() <= CloudFilters::CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH as usize,
            "blob size must not exceed {} bytes, got {} bytes",
            CloudFilters::CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH,
            blob.len()
        );
        self.blob = Some(blob);
        self
    }
}

impl Default for ConvertOptions<'_> {
    fn default() -> Self {
        Self {
            flags: CloudFilters::CF_CONVERT_FLAG_NONE,
            blob: None,
        }
    }
}

/// Placeholder update parameters.
#[derive(Debug, Clone)]
pub struct UpdateOptions<'a> {
    metadata: Option<Metadata>,
    dehydrate_range: Vec<CF_FILE_RANGE>,
    flags: CF_UPDATE_FLAGS,
    blob: Option<&'a [u8]>,
}

impl<'a> UpdateOptions<'a> {
    ///
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    // TODO: user should be able to specify an array of RangeBounds
    pub fn dehydrate_range(mut self, range: Range<u64>) -> Self {
        self.dehydrate_range.push(CF_FILE_RANGE {
            StartingOffset: range.start as i64,
            Length: range.end as i64,
        });
        self
    }

    pub fn update_if_synced(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_VERIFY_IN_SYNC;
        self
    }

    pub fn mark_sync(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_MARK_IN_SYNC;
        self
    }

    // files only
    pub fn dehydrate(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_DEHYDRATE;
        self
    }

    // directories only
    pub fn children_present(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_DISABLE_ON_DEMAND_POPULATION;
        self
    }

    pub fn remove_blob(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_REMOVE_FILE_IDENTITY;
        self
    }

    pub fn mark_unsync(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_CLEAR_IN_SYNC;
        self
    }

    // TODO: what does this do?
    pub fn remove_properties(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_REMOVE_PROPERTY;
        self
    }

    // TODO: this doesn't seem necessary
    pub fn skip_0_metadata_fields(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_PASSTHROUGH_FS_METADATA;
        self
    }

    pub fn blob(mut self, blob: &'a [u8]) -> Self {
        assert!(
            blob.len() <= CloudFilters::CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH as usize,
            "blob size must not exceed {} bytes, got {} bytes",
            CloudFilters::CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH,
            blob.len()
        );
        self.blob = Some(blob);
        self
    }
}

impl Default for UpdateOptions<'_> {
    fn default() -> Self {
        Self {
            metadata: None,
            dehydrate_range: Vec::new(),
            flags: CloudFilters::CF_UPDATE_FLAG_NONE
                | CloudFilters::CF_UPDATE_FLAG_ENABLE_ON_DEMAND_POPULATION,
            blob: None,
        }
    }
}

// TODO: I don't think this is an enum
#[derive(Debug, Clone, Copy)]
pub enum PlaceholderState {
    Placeholder,
    SyncRoot,
    EssentialPropPresent,
    InSync,
    StatePartial,
    PartiallyOnDisk,
}

impl PlaceholderState {
    fn try_from_win32(value: CF_PLACEHOLDER_STATE) -> core::Result<Option<PlaceholderState>> {
        match value {
            CloudFilters::CF_PLACEHOLDER_STATE_NO_STATES => Ok(None),
            CloudFilters::CF_PLACEHOLDER_STATE_PLACEHOLDER => Ok(Some(Self::Placeholder)),
            CloudFilters::CF_PLACEHOLDER_STATE_SYNC_ROOT => Ok(Some(Self::SyncRoot)),
            CloudFilters::CF_PLACEHOLDER_STATE_ESSENTIAL_PROP_PRESENT => {
                Ok(Some(Self::EssentialPropPresent))
            }
            CloudFilters::CF_PLACEHOLDER_STATE_IN_SYNC => Ok(Some(Self::InSync)),
            CloudFilters::CF_PLACEHOLDER_STATE_PARTIAL => Ok(Some(Self::StatePartial)),
            CloudFilters::CF_PLACEHOLDER_STATE_PARTIALLY_ON_DISK => Ok(Some(Self::PartiallyOnDisk)),
            CloudFilters::CF_PLACEHOLDER_STATE_INVALID => Err(core::Error::from_win32()),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub struct PlaceholderInfo {
    data: Vec<u8>,
    info: *const CF_PLACEHOLDER_STANDARD_INFO,
}

impl PlaceholderInfo {
    pub fn on_disk_data_size(&self) -> u64 {
        unsafe { &*self.info }.OnDiskDataSize as u64
    }

    pub fn validated_data_size(&self) -> u64 {
        unsafe { &*self.info }.ValidatedDataSize as u64
    }
    pub fn modified_data_size(&self) -> u64 {
        unsafe { &*self.info }.ModifiedDataSize as u64
    }
    pub fn properties_size(&self) -> u64 {
        unsafe { &*self.info }.PropertiesSize as u64
    }

    pub fn pin_state(&self) -> PinState {
        unsafe { &*self.info }.PinState.into()
    }

    pub fn is_synced(&self) -> bool {
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
