use std::{fmt::Debug, ops::Range, path::PathBuf};

use widestring::U16CStr;
use windows::Win32::Storage::CloudFilters::{
    self, CF_CALLBACK_CANCEL_FLAGS, CF_CALLBACK_DEHYDRATION_REASON, CF_CALLBACK_PARAMETERS_0_0,
    CF_CALLBACK_PARAMETERS_0_1, CF_CALLBACK_PARAMETERS_0_10, CF_CALLBACK_PARAMETERS_0_11,
    CF_CALLBACK_PARAMETERS_0_2, CF_CALLBACK_PARAMETERS_0_3, CF_CALLBACK_PARAMETERS_0_4,
    CF_CALLBACK_PARAMETERS_0_5, CF_CALLBACK_PARAMETERS_0_6, CF_CALLBACK_PARAMETERS_0_7,
    CF_CALLBACK_PARAMETERS_0_8, CF_CALLBACK_PARAMETERS_0_9,
};

/// Information for the [SyncFilter::fetch_data][crate::SyncFilter::fetch_data] callback.
#[derive(Debug, Clone, Copy)]
pub struct FetchData(pub(crate) CF_CALLBACK_PARAMETERS_0_6);

impl FetchData {
    /// Whether or not the callback was called from an interrupted hydration.
    pub fn interrupted_hydration(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_FETCH_DATA_FLAG_RECOVERY).0 != 0
    }

    /// Whether or not the callback was called from an explicit hydration via
    /// [FileExt::hydrate][crate::ext::FileExt::hydrate].
    pub fn explicit_hydration(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_FETCH_DATA_FLAG_EXPLICIT_HYDRATION).0 != 0
    }

    // TODO: does this amount always lay on 4kb or EoF?
    /// The amount of bytes that must be written to the placeholder.
    pub fn required_file_range(&self) -> Range<u64> {
        (self.0.RequiredFileOffset as u64)
            ..(self.0.RequiredFileOffset + self.0.RequiredLength) as u64
    }

    // TODO: what is this field
    // https://docs.microsoft.com/en-us/answers/questions/748214/what-is-fetchdataoptionalfileoffset-cfapi.html
    pub fn optional_file_range(&self) -> Range<u64> {
        (self.0.OptionalFileOffset as u64)
            ..(self.0.OptionalFileOffset + self.0.OptionalLength) as u64
    }

    /// The last time the file was dehydrated.
    ///
    /// This value is a count of 100-nanosecond intervals since January 1, 1601.
    pub fn last_dehydration_time(&self) -> u64 {
        self.0.LastDehydrationTime as u64
    }

    /// The reason the file was last dehydrated.
    pub fn last_dehydration_reason(&self) -> Option<DehydrationReason> {
        DehydrationReason::from_win32(self.0.LastDehydrationReason)
    }
}

/// Information for the [SyncFilter::cancel_fetch_data][crate::SyncFilter::cancel_fetch_data] callback.
#[derive(Clone, Copy)]
pub struct CancelFetchData(pub(crate) CF_CALLBACK_PARAMETERS_0_0);

impl CancelFetchData {
    /// Whether or not the callback failed as a result of the 60 second timeout.
    ///
    /// Read more [here][crate::Request::reset_timeout].
    pub fn timeout(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_CANCEL_FLAG_IO_TIMEOUT).0 != 0
    }

    /// The user has cancelled the request manually.
    ///
    /// A user could cancel a request through a download toast?
    pub fn user_cancelled(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_CANCEL_FLAG_IO_ABORTED).0 != 0
    }

    /// The range of data that was supposed to be fetched.
    pub fn file_range(&self) -> Range<u64> {
        let range = unsafe { self.0.Anonymous.FetchData };
        (range.FileOffset as u64)..(range.FileOffset + range.Length) as u64
    }
}

impl Debug for CancelFetchData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CancelFetchData")
            .field(unsafe {
                &CancelFetchDataDebug {
                    Flags: self.0.Flags,
                    FileOffset: self.0.Anonymous.FetchData.FileOffset,
                    Length: self.0.Anonymous.FetchData.Length,
                }
            })
            .finish()
    }
}

#[allow(dead_code, non_snake_case)]
#[derive(Debug)]
struct CancelFetchDataDebug {
    Flags: CF_CALLBACK_CANCEL_FLAGS,
    FileOffset: i64,
    Length: i64,
}

/// Information for the [SyncFilter::validate_data][crate::SyncFilter::validate_data] callback.
#[derive(Debug, Clone, Copy)]
pub struct ValidateData(pub(crate) CF_CALLBACK_PARAMETERS_0_11);

impl ValidateData {
    /// Whether or not the callback failed as a result of the 60 second timeout.
    pub fn explicit_hydration(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_VALIDATE_DATA_FLAG_EXPLICIT_HYDRATION).0 != 0
    }

    /// The range of data to validate.
    pub fn file_range(&self) -> Range<u64> {
        (self.0.RequiredFileOffset as u64)
            ..(self.0.RequiredFileOffset + self.0.RequiredLength) as u64
    }
}

/// Information for the [SyncFilter::fetch_placeholders][crate::SyncFilter::fetch_placeholders]
/// callback.
#[derive(Debug)]
pub struct FetchPlaceholders(pub(crate) CF_CALLBACK_PARAMETERS_0_7);

impl FetchPlaceholders {
    /// A glob pattern specifying the files that should be fetched.
    ///
    /// This field is completely optional and does not have to be respected.
    #[cfg(feature = "globs")]
    pub fn pattern(&self) -> Result<globset::Glob, globset::Error> {
        let pattern = unsafe { U16CStr::from_ptr_str(self.0.Pattern.0) }.to_string_lossy();
        globset::Glob::new(&pattern)
    }

    /// A glob pattern specifying the files that should be fetched.
    ///
    /// This field is completely optional and does not have to be respected.
    #[cfg(not(feature = "globs"))]
    pub fn pattern(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.0.Pattern.0) }
    }
}

/// Information for the
/// [SyncFilter::cancel_fetch_placeholders][crate::SyncFilter::cancel_fetch_placeholders] callback.
#[derive(Clone, Copy)]
pub struct CancelFetchPlaceholders(pub(crate) CF_CALLBACK_PARAMETERS_0_0);

impl CancelFetchPlaceholders {
    /// Whether or not the callback failed as a result of the 60 second timeout.
    ///
    /// Read more [here][crate::Request::reset_timeout].
    pub fn timeout(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_CANCEL_FLAG_IO_TIMEOUT).0 != 0
    }

    /// The user has cancelled the request manually.
    ///
    /// A user could cancel a request through a download toast?
    pub fn user_cancelled(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_CANCEL_FLAG_IO_ABORTED).0 != 0
    }
}

impl Debug for CancelFetchPlaceholders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CancelFetchPlaceholders")
            .field(&CancelFetchPlaceholdersDebug {
                Flags: self.0.Flags,
            })
            .finish()
    }
}

#[allow(dead_code, non_snake_case)]
#[derive(Debug)]
struct CancelFetchPlaceholdersDebug {
    Flags: CF_CALLBACK_CANCEL_FLAGS,
}

/// Information for the [SyncFilter::opened][crate::SyncFilter::opened] callback.
#[derive(Debug, Clone, Copy)]
pub struct Opened(pub(crate) CF_CALLBACK_PARAMETERS_0_8);

impl Opened {
    /// The placeholder metadata is corrupt.
    pub fn metadata_corrupt(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_OPEN_COMPLETION_FLAG_PLACEHOLDER_UNKNOWN).0 != 0
    }

    /// The placeholder metadata is not supported.
    pub fn metadata_unsupported(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_OPEN_COMPLETION_FLAG_PLACEHOLDER_UNSUPPORTED).0
            != 0
    }
}

/// Information for the [SyncFilter::closed][crate::SyncFilter::closed] callback.
#[derive(Debug, Clone, Copy)]
pub struct Closed(pub(crate) CF_CALLBACK_PARAMETERS_0_1);

impl Closed {
    /// Whether or not the placeholder was deleted as a result of the close.
    pub fn deleted(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_CLOSE_COMPLETION_FLAG_DELETED).0 != 0
    }
}

/// Information for the [SyncFilter::dehydrate][crate::SyncFilter::dehydrate] callback.
#[derive(Debug, Clone, Copy)]
pub struct Dehydrate(pub(crate) CF_CALLBACK_PARAMETERS_0_3);

impl Dehydrate {
    /// Whether or not the callback was called from a system background service.
    pub fn background(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_DEHYDRATE_FLAG_BACKGROUND).0 != 0
    }

    /// The reason the file is being dehydrated.
    pub fn reason(&self) -> Option<DehydrationReason> {
        DehydrationReason::from_win32(self.0.Reason)
    }
}

/// Information for the [SyncFilter::dehydrated][crate::SyncFilter::dehydrated] callback.
#[derive(Debug, Clone, Copy)]
pub struct Dehydrated(pub(crate) CF_CALLBACK_PARAMETERS_0_2);

impl Dehydrated {
    /// Whether or not the callback was called from a system background service.
    pub fn background(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_DEHYDRATE_COMPLETION_FLAG_BACKGROUND).0 != 0
    }

    /// Whether or not the placeholder was already hydrated.
    pub fn already_hydrated(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_DEHYDRATE_COMPLETION_FLAG_DEHYDRATED).0 != 0
    }

    /// The reason the file is being dehydrated.
    pub fn reason(&self) -> Option<DehydrationReason> {
        DehydrationReason::from_win32(self.0.Reason)
    }
}

/// Information for the [SyncFilter::delete][crate::SyncFilter::delete] callback.
#[derive(Debug, Clone, Copy)]
pub struct Delete(pub(crate) CF_CALLBACK_PARAMETERS_0_5);

impl Delete {
    /// Whether or not the placeholder being deleted is a directory.
    pub fn is_directory(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_DELETE_FLAG_IS_DIRECTORY).0 != 0
    }

    // TODO: missing docs
    pub fn is_undelete(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_DELETE_FLAG_IS_UNDELETE).0 != 0
    }
}

/// Information for the [SyncFilter::deleted][crate::SyncFilter::deleted] callback.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Deleted(pub(crate) CF_CALLBACK_PARAMETERS_0_4);

/// Information for the [SyncFilter::rename][crate::SyncFilter::rename] callback.
#[derive(Debug)]
pub struct Rename(pub(crate) CF_CALLBACK_PARAMETERS_0_10);

impl Rename {
    /// Whether or not the placeholder being deleted is a directory.
    pub fn is_directory(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_RENAME_FLAG_IS_DIRECTORY).0 != 0
    }

    /// Whether or not the placeholder was originally in the sync root.
    pub fn source_in_scope(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_RENAME_FLAG_SOURCE_IN_SCOPE).0 != 0
    }

    /// Whether or not the placeholder is being moved inside the sync root.
    pub fn target_in_scope(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_RENAME_FLAG_TARGET_IN_SCOPE).0 != 0
    }

    /// The full path the placeholder is being moved to.
    pub fn target_path(&self) -> PathBuf {
        unsafe {
            U16CStr::from_ptr_str(self.0.TargetPath.0)
                .to_os_string()
                .into()
        }
    }
}

/// Information for the [SyncFilter::renamed][crate::SyncFilter::renamed] callback.
#[derive(Debug)]
pub struct Renamed(pub(crate) CF_CALLBACK_PARAMETERS_0_9);

impl Renamed {
    /// The full path the placeholder has been moved from.
    pub fn source_path(&self) -> PathBuf {
        unsafe {
            U16CStr::from_ptr_str(self.0.SourcePath.0)
                .to_os_string()
                .into()
        }
    }
}

/// The reason a placeholder has been dehydrated.
#[derive(Debug, Clone, Copy)]
pub enum DehydrationReason {
    /// The user manually dehydrated the placeholder.
    UserManually,
    /// The operating system automatically dehydrated the placeholder due to low disk space on the
    /// volume.
    LowSpace,
    /// The operating system automatically dehydrated the placeholder due to low activity.
    ///
    /// This is based on the Windows Storage Sense settings.
    Inactive,
    /// The operating system automatically dehydrated this file to make room for an operating
    /// system upgrade.
    OsUpgrade,
}

impl DehydrationReason {
    fn from_win32(reason: CF_CALLBACK_DEHYDRATION_REASON) -> Option<DehydrationReason> {
        match reason {
            CloudFilters::CF_CALLBACK_DEHYDRATION_REASON_USER_MANUAL => Some(Self::UserManually),
            CloudFilters::CF_CALLBACK_DEHYDRATION_REASON_SYSTEM_LOW_SPACE => Some(Self::LowSpace),
            CloudFilters::CF_CALLBACK_DEHYDRATION_REASON_SYSTEM_INACTIVITY => Some(Self::Inactive),
            CloudFilters::CF_CALLBACK_DEHYDRATION_REASON_SYSTEM_OS_UPGRADE => Some(Self::OsUpgrade),
            _ => None,
        }
    }
}
