use std::{
    fs::File,
    mem::{self, MaybeUninit},
    ops::{Bound, RangeBounds},
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
                self, CfDehydratePlaceholder, CfGetPlaceholderRangeInfo,
                CfGetPlaceholderStateFromFileInfo, CfGetSyncRootInfoByHandle, CfHydratePlaceholder,
                CfSetInSyncState, CF_PLACEHOLDER_RANGE_INFO_CLASS, CF_PLACEHOLDER_STATE,
                CF_SYNC_PROVIDER_STATUS, CF_SYNC_ROOT_STANDARD_INFO,
            },
            FileSystem::{self, GetFileInformationByHandleEx, FILE_ATTRIBUTE_TAG_INFO},
        },
    },
};

use crate::{
    root::{HydrationPolicy, HydrationType, PopulationType, SupportedAttributes},
    usn::Usn,
};

/// An API extension to [File][std::fs::File].
pub trait FileExt: AsRawHandle {
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

    #[allow(clippy::missing_safety_doc)]
    /// Gets various characteristics of a placeholder using the passed blob size.
    unsafe fn sync_root_info_unchecked(&self, blob_size: usize) -> core::Result<SyncRootInfo> {
        let mut data = vec![0; mem::size_of::<CF_SYNC_ROOT_STANDARD_INFO>() + blob_size];

        unsafe {
            CfGetSyncRootInfoByHandle(
                HANDLE(self.as_raw_handle() as isize),
                CloudFilters::CF_SYNC_ROOT_INFO_STANDARD,
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
