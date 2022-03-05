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
                CfGetPlaceholderStateFromFileInfo, CfGetSyncRootInfoByHandle, CfHydratePlaceholder,
                CfRevertPlaceholder, CfSetInSyncState, CfSetPinState, CfUpdatePlaceholder,
                CF_CONVERT_FLAGS, CF_FILE_RANGE, CF_PIN_STATE, CF_PLACEHOLDER_STANDARD_INFO,
                CF_PLACEHOLDER_STATE, CF_SET_PIN_FLAGS, CF_SYNC_PROVIDER_STATUS,
                CF_SYNC_ROOT_INFO_STANDARD, CF_SYNC_ROOT_STANDARD_INFO, CF_UPDATE_FLAGS,
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

pub trait FileExt: AsRawHandle {
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

    // must have write perms
    fn to_file(&self) -> core::Result<()> {
        unsafe {
            CfRevertPlaceholder(
                HANDLE(self.as_raw_handle() as isize),
                CloudFilters::CF_REVERT_FLAG_NONE,
                ptr::null_mut(),
            )
        }
    }

    // this could be split into multiple functions to make common patterns easier
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

    fn hydrate<T: RangeBounds<u64>>(&self, range: T) -> core::Result<()> {
        unsafe {
            CfHydratePlaceholder(
                HANDLE(self.as_raw_handle() as isize),
                match range.start_bound() {
                    Bound::Included(x) => *x as i64,
                    Bound::Excluded(x) => x.saturating_add(1) as i64,
                    Bound::Unbounded => 0,
                } as i64,
                match range.end_bound() {
                    Bound::Included(x) => *x as i64,
                    Bound::Excluded(x) => x.saturating_sub(1) as i64,
                    Bound::Unbounded => -1,
                } as i64,
                CloudFilters::CF_HYDRATE_FLAG_NONE,
                ptr::null_mut(),
            )
        }
    }

    fn dehydrate<T: RangeBounds<u64>>(&self, range: T) -> core::Result<()> {
        dehydrate(self.as_raw_handle(), range, false)
    }

    fn background_dehydrate<T: RangeBounds<u64>>(&self, range: T) -> core::Result<()> {
        dehydrate(self.as_raw_handle(), range, true)
    }

    fn placeholder_info(&self) -> core::Result<PlaceholderInfo> {
        // TODO: same as below except finds the size after 2 calls of CfGetPlaceholderInfo
        todo!()
    }

    /// # Safety
    /// `blob_size` must be the size of the file blob.
    unsafe fn placeholder_info_unchecked(&self, blob_size: usize) -> core::Result<PlaceholderInfo> {
        let mut data = vec![0; mem::size_of::<CF_PLACEHOLDER_STANDARD_INFO>() + blob_size];

        CfGetPlaceholderInfo(
            HANDLE(self.as_raw_handle() as isize),
            CloudFilters::CF_PLACEHOLDER_INFO_STANDARD,
            data.as_mut_ptr() as *mut _,
            data.len() as u32,
            ptr::null_mut(),
        )?;

        Ok(PlaceholderInfo {
            info: &data[..=mem::size_of::<CF_PLACEHOLDER_STANDARD_INFO>()]
                .align_to::<CF_PLACEHOLDER_STANDARD_INFO>()
                .1[0] as *const _,
            data,
        })
    }

    // if it fails, it will be Err,
    // if it is not a placeholder, then it will be None
    // it should instead error with not a placeholder
    // TODO: why is the value showing 9 in my tests, 9 is not a valid enum?
    // does it return flags?
    fn placeholder_state(&self) -> core::Result<Option<PlaceholderState>> {
        let mut info = MaybeUninit::<FILE_ATTRIBUTE_TAG_INFO>::uninit();
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

    // TODO: make a type for Usn's
    fn mark_sync(&self, usn: Usn) -> core::Result<Usn> {
        mark_sync_state(self.as_raw_handle(), true, usn)
    }

    fn mark_unsync(&self, usn: Usn) -> core::Result<Usn> {
        mark_sync_state(self.as_raw_handle(), false, usn)
    }

    fn is_placeholder(&self) -> bool {
        match self.placeholder_state() {
            Ok(state) => state.is_some(),
            Err(..) => false,
        }
    }

    fn sync_root_info(&self) -> core::Result<SyncRootInfo> {
        // TODO: this except finds the size after 2 calls of CfGetSyncRootInfoByHandle
        todo!()
    }

    // TODO: create a return value for this
    /// # Safety
    /// `blob_size` must be the size of the register blob.
    unsafe fn sync_root_info_unchecked(&self, blob_size: usize) -> core::Result<SyncRootInfo> {
        let mut data = vec![0; mem::size_of::<CF_SYNC_ROOT_STANDARD_INFO>() + blob_size];

        CfGetSyncRootInfoByHandle(
            HANDLE(self.as_raw_handle() as isize),
            CF_SYNC_ROOT_INFO_STANDARD,
            data.as_mut_ptr() as *mut _,
            data.len() as u32,
            ptr::null_mut(),
        )?;

        Ok(SyncRootInfo {
            info: &data[..=mem::size_of::<CF_SYNC_ROOT_STANDARD_INFO>()]
                .align_to::<CF_SYNC_ROOT_STANDARD_INFO>()
                .1[0] as *const _,
            data,
        })
    }

    fn in_sync_root() -> bool {
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

#[derive(Debug)]
pub struct SyncRootInfo {
    data: Vec<u8>,
    info: *const CF_SYNC_ROOT_STANDARD_INFO,
}

impl SyncRootInfo {
    pub fn file_id(&self) -> u64 {
        unsafe { &*self.info }.SyncRootFileId as u64
    }

    pub fn hydration_policy(&self) -> HydrationType {
        unsafe { &*self.info }.HydrationPolicy.Primary.into()
    }

    pub fn hydration_type(&self) -> HydrationPolicy {
        unsafe { &*self.info }.HydrationPolicy.Modifier.into()
    }

    pub fn population_type(&self) -> PopulationType {
        unsafe { &*self.info }.PopulationPolicy.Primary.into()
    }

    pub fn supported_attributes(&self) -> SupportedAttributes {
        unsafe { &*self.info }.InSyncPolicy.into()
    }

    pub fn hardlinks_allowed(&self) -> bool {
        unsafe { &*self.info }.HardLinkPolicy == CloudFilters::CF_HARDLINK_POLICY_ALLOWED
    }

    pub fn status(&self) -> ProviderStatus {
        unsafe { &*self.info }.ProviderStatus.into()
    }

    pub fn provider_name(&self) -> &U16CStr {
        U16CStr::from_slice_truncate(unsafe { &*self.info }.ProviderName.as_slice()).unwrap()
    }

    pub fn version(&self) -> &U16CStr {
        U16CStr::from_slice_truncate(unsafe { &*self.info }.ProviderVersion.as_slice()).unwrap()
    }

    pub fn blob(&self) -> &[u8] {
        &self.data[(mem::size_of::<CF_SYNC_ROOT_STANDARD_INFO>() + 1)..]
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProviderStatus {
    Disconnected,
    Idle,
    PopulateNamespace,
    PopulateMetadata,
    PopulateContent,
    SyncIncremental,
    SyncFull,
    ConnectivityLost,
    ClearFlags,
    Terminated,
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
            CloudFilters::CF_PROVIDER_STATUS_CLEAR_FLAGS => Self::ClearFlags,
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
            ProviderStatus::ClearFlags => CloudFilters::CF_PROVIDER_STATUS_CLEAR_FLAGS,
            ProviderStatus::Terminated => CloudFilters::CF_PROVIDER_STATUS_TERMINATED,
            ProviderStatus::Error => CloudFilters::CF_PROVIDER_STATUS_ERROR,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PinState {
    Unspecified,
    Pinned,
    Unpinned,
    Excluded,
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

#[derive(Debug, Clone, Copy)]
pub struct PinOptions(CF_SET_PIN_FLAGS);

impl PinOptions {
    pub fn pin_descendants(&mut self) -> &mut Self {
        self.0 |= CloudFilters::CF_SET_PIN_FLAG_RECURSE;
        self
    }

    pub fn pin_descendants_not_self(&mut self) -> &mut Self {
        self.0 |= CloudFilters::CF_SET_PIN_FLAG_RECURSE_ONLY;
        self
    }

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

#[derive(Debug, Clone)]
pub struct ConvertOptions<'a> {
    flags: CF_CONVERT_FLAGS,
    blob: Option<&'a [u8]>,
}

impl<'a> ConvertOptions<'a> {
    pub fn mark_sync(mut self) -> Self {
        self.flags |= CloudFilters::CF_CONVERT_FLAG_MARK_IN_SYNC;
        self
    }

    // can only be called for files
    pub fn dehydrate(mut self) -> Self {
        self.flags |= CloudFilters::CF_CONVERT_FLAG_DEHYDRATE;
        self
    }

    // can only be called for directories
    pub fn children_not_present(mut self) -> Self {
        self.flags |= CloudFilters::CF_CONVERT_FLAG_ENABLE_ON_DEMAND_POPULATION;
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

    // TODO: missing docs CF_CONVERT_FLAGS
    // https://docs.microsoft.com/en-us/answers/questions/749972/missing-documentation-in-cf-convert-flags-cfapi.html

    // pub fn always_full(mut self) -> Self {
    //     self.flags |= CloudFilters::CF_CONVERT_FLAG_ALWAYS_FULL;
    //     self
    // }

    // pub fn convert_to_cloud_file(mut self) -> Self {
    //     set_flag(
    //         &mut self.flags,
    //         CloudFilters::CF_CONVERT_FLAG_FORCE_CONVERT_TO_CLOUD_FILE,
    //         yes,
    //     );
    //     self
    // }
}

impl Default for ConvertOptions<'_> {
    fn default() -> Self {
        Self {
            flags: CloudFilters::CF_CONVERT_FLAG_NONE,
            blob: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct UpdateOptions<'a> {
    metadata: Option<Metadata>,
    dehydrate_range: Vec<CF_FILE_RANGE>,
    flags: CF_UPDATE_FLAGS,
    blob: Option<&'a [u8]>,
}

impl<'a> UpdateOptions<'a> {
    #[must_use]
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    // TODO: user should be able to specify an array of RangeBounds
    #[must_use]
    pub fn dehydrate_range(mut self, range: Range<u64>) -> Self {
        self.dehydrate_range.push(CF_FILE_RANGE {
            StartingOffset: range.start as i64,
            Length: range.end as i64,
        });
        self
    }

    #[must_use]
    pub fn update_if_synced(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_VERIFY_IN_SYNC;
        self
    }

    #[must_use]
    pub fn mark_sync(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_MARK_IN_SYNC;
        self
    }

    // files only
    #[must_use]
    pub fn dehydrate(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_DEHYDRATE;
        self
    }

    // directories only
    #[must_use]
    pub fn children_present(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_DISABLE_ON_DEMAND_POPULATION;
        self
    }

    #[must_use]
    pub fn remove_blob(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_REMOVE_FILE_IDENTITY;
        self
    }

    #[must_use]
    pub fn mark_unsync(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_CLEAR_IN_SYNC;
        self
    }

    // TODO: what does this do?
    #[must_use]
    pub fn remove_properties(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_REMOVE_PROPERTY;
        self
    }

    // TODO: this doesn't seem necessary
    #[must_use]
    pub fn skip_0_metadata_fields(mut self) -> Self {
        self.flags |= CloudFilters::CF_UPDATE_FLAG_PASSTHROUGH_FS_METADATA;
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
        &self.data[(mem::size_of::<CF_PLACEHOLDER_STANDARD_INFO>() + 1)..]
    }
}
