use std::{
    fs::File,
    mem::{self, MaybeUninit},
    ops::{Bound, Range, RangeBounds},
    os::windows::io::AsRawHandle,
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
                CfRevertPlaceholder, CfSetPinState, CfUpdatePlaceholder, CF_CONVERT_FLAGS,
                CF_FILE_RANGE, CF_PIN_STATE, CF_PLACEHOLDER_STANDARD_INFO, CF_PLACEHOLDER_STATE,
                CF_SET_PIN_FLAGS, CF_SYNC_PROVIDER_STATUS, CF_SYNC_ROOT_INFO_STANDARD,
                CF_SYNC_ROOT_STANDARD_INFO, CF_UPDATE_FLAGS,
            },
            FileSystem::{self, GetFileInformationByHandleEx, FILE_ATTRIBUTE_TAG_INFO},
        },
    },
};

use crate::{
    placeholder_file::Metadata,
    root::{
        register::{HydrationPolicy, HydrationType, InSyncPolicy, PopulationType},
        set_flag,
    },
};

// TODO: Support file identities
pub trait FileExt: AsRawHandle {
    fn to_placeholder(&self, options: ConvertOptions) -> core::Result<u64> {
        let mut usn = MaybeUninit::<i64>::uninit();
        unsafe {
            CfConvertToPlaceholder(
                HANDLE(self.as_raw_handle() as isize),
                ptr::null(),
                0,
                options.0,
                usn.as_mut_ptr(),
                ptr::null_mut(),
            )
            .map(|_| usn.assume_init() as u64)
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

    // TODO: this should be split up into multiple functions
    fn update_placeholder(&self, mut options: UpdateOptions) -> core::Result<Option<u64>> {
        unsafe {
            CfUpdatePlaceholder(
                HANDLE(self.as_raw_handle() as isize),
                options.metadata.map_or(ptr::null(), |x| &x.0 as *const _),
                options.blob.map_or(ptr::null(), |x| x.as_ptr() as *const _),
                options.blob.map_or(0, |x| x.len() as u32),
                options.dehydrate_range.as_mut_ptr(),
                options.dehydrate_range.len() as u32,
                options.flags,
                options.usn.map_or(ptr::null_mut(), |x| x as *mut i64),
                ptr::null_mut(),
            )
            .map(|_| options.usn.map(|x| x as u64))
        }
    }

    fn hydrate_placeholder<T: RangeBounds<u64>>(&self, range: T) -> core::Result<()> {
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

    // TODO: create two separate functions for the background param
    fn dehydrate_placeholder<T: RangeBounds<u64>>(
        &self,
        range: T,
        background: bool,
    ) -> core::Result<()> {
        // self.update_placeholder(UpdateOptions::default().dehydrate_range(range))?;
        unsafe {
            // TODO: is this function deprecated or not?
            CfDehydratePlaceholder(
                HANDLE(self.as_raw_handle() as isize),
                // TODO: These bounds checks and behavior could be abstracted into a separate struct. Do other API's that require ranges
                // follow the same conventions?
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

    fn placeholder_info(&self) -> core::Result<PlaceholderInfo> {
        // TODO: this except finds the size after 2 calls of CfGetPlaceholderInfo
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
    // TODO: why is the value showing 9 in my tests, 9 is not a valid enum?
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

    fn is_in_sync_root() -> bool {
        // TODO: this
        todo!()
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

    pub fn in_sync_policy(&self) -> InSyncPolicy {
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
    pub fn recurse(&mut self, yes: bool) -> &mut Self {
        set_flag(&mut self.0, CloudFilters::CF_SET_PIN_FLAG_RECURSE, yes);
        self
    }

    pub fn recurse_only(&mut self, yes: bool) -> &mut Self {
        set_flag(&mut self.0, CloudFilters::CF_SET_PIN_FLAG_RECURSE_ONLY, yes);
        self
    }

    pub fn stop_on_error(&mut self, yes: bool) -> &mut Self {
        set_flag(
            &mut self.0,
            CloudFilters::CF_SET_PIN_FLAG_RECURSE_STOP_ON_ERROR,
            yes,
        );
        self
    }
}

impl Default for PinOptions {
    fn default() -> Self {
        Self(CloudFilters::CF_SET_PIN_FLAG_NONE)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ConvertOptions(CF_CONVERT_FLAGS);

impl ConvertOptions {
    pub fn mark_in_sync(&mut self, yes: bool) -> &mut Self {
        set_flag(&mut self.0, CloudFilters::CF_CONVERT_FLAG_MARK_IN_SYNC, yes);
        self
    }

    // TODO: can only be called for files
    pub fn dehydrate(&mut self, yes: bool) -> &mut Self {
        set_flag(&mut self.0, CloudFilters::CF_CONVERT_FLAG_DEHYDRATE, yes);
        self
    }

    // TODO: can only be called for directories
    pub fn on_demand_population(&mut self, yes: bool) -> &mut Self {
        set_flag(
            &mut self.0,
            CloudFilters::CF_CONVERT_FLAG_ENABLE_ON_DEMAND_POPULATION,
            yes,
        );
        self
    }

    pub fn always_full(&mut self, yes: bool) -> &mut Self {
        set_flag(&mut self.0, CloudFilters::CF_CONVERT_FLAG_ALWAYS_FULL, yes);
        self
    }

    pub fn convert_to_cloud_file(&mut self, yes: bool) -> &mut Self {
        set_flag(
            &mut self.0,
            CloudFilters::CF_CONVERT_FLAG_FORCE_CONVERT_TO_CLOUD_FILE,
            yes,
        );
        self
    }
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self(CloudFilters::CF_CONVERT_FLAG_NONE)
    }
}

#[derive(Debug, Clone)]
pub struct UpdateOptions<'a> {
    metadata: Option<Metadata>,
    dehydrate_range: Vec<CF_FILE_RANGE>,
    flags: CF_UPDATE_FLAGS,
    usn: Option<u64>,
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
    pub fn usn(mut self, usn: u64) -> Self {
        self.usn = Some(usn);
        self
    }

    #[must_use]
    pub fn verify_in_sync(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.flags,
            CloudFilters::CF_UPDATE_FLAG_VERIFY_IN_SYNC,
            yes,
        );
        self
    }

    #[must_use]
    pub fn mark_in_sync(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.flags,
            CloudFilters::CF_UPDATE_FLAG_MARK_IN_SYNC,
            yes,
        );
        self
    }

    // TODO: files only
    #[must_use]
    pub fn dehydrate(mut self, yes: bool) -> Self {
        set_flag(&mut self.flags, CloudFilters::CF_UPDATE_FLAG_DEHYDRATE, yes);
        self
    }

    // TODO: directories only
    #[must_use]
    pub fn on_demand_population(mut self, yes: bool) -> Self {
        if yes {
            self.flags |= CloudFilters::CF_UPDATE_FLAG_ENABLE_ON_DEMAND_POPULATION;
            self.flags &= !CloudFilters::CF_UPDATE_FLAG_DISABLE_ON_DEMAND_POPULATION;
        } else {
            self.flags |= CloudFilters::CF_UPDATE_FLAG_DISABLE_ON_DEMAND_POPULATION;
            self.flags &= !CloudFilters::CF_UPDATE_FLAG_ENABLE_ON_DEMAND_POPULATION;
        }

        self
    }

    #[must_use]
    pub fn remove_file_identity(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.flags,
            CloudFilters::CF_UPDATE_FLAG_REMOVE_FILE_IDENTITY,
            yes,
        );
        self
    }

    #[must_use]
    pub fn clear_in_sync(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.flags,
            CloudFilters::CF_UPDATE_FLAG_CLEAR_IN_SYNC,
            yes,
        );
        self
    }

    #[must_use]
    pub fn remove_property(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.flags,
            CloudFilters::CF_UPDATE_FLAG_REMOVE_PROPERTY,
            yes,
        );
        self
    }

    #[must_use]
    pub fn passthrough_fs_metadata(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.flags,
            CloudFilters::CF_UPDATE_FLAG_PASSTHROUGH_FS_METADATA,
            yes,
        );
        self
    }

    #[must_use]
    pub fn blob(mut self, blob: &'a [u8]) -> Self {
        self.blob = Some(blob);
        self
    }
}

impl Default for UpdateOptions<'_> {
    fn default() -> Self {
        Self {
            metadata: None,
            dehydrate_range: Vec::new(),
            flags: CloudFilters::CF_UPDATE_FLAG_NONE,
            usn: None,
            blob: None,
        }
    }
}

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

    pub fn blob(&self) -> Option<&[u8]> {
        let start = mem::size_of::<CF_PLACEHOLDER_STANDARD_INFO>() + 1;
        match self.data.len() - start {
            0 => None,
            _ => Some(&self.data[start..]),
        }
    }
}
