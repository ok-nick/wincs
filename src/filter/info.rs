use std::{ops::Range, path::PathBuf};

use widestring::U16CStr;
use windows::Win32::Storage::CloudFilters::{
    self, CF_CALLBACK_DEHYDRATION_REASON, CF_CALLBACK_PARAMETERS_0_0, CF_CALLBACK_PARAMETERS_0_1,
    CF_CALLBACK_PARAMETERS_0_10, CF_CALLBACK_PARAMETERS_0_11, CF_CALLBACK_PARAMETERS_0_2,
    CF_CALLBACK_PARAMETERS_0_3, CF_CALLBACK_PARAMETERS_0_4, CF_CALLBACK_PARAMETERS_0_5,
    CF_CALLBACK_PARAMETERS_0_6, CF_CALLBACK_PARAMETERS_0_7, CF_CALLBACK_PARAMETERS_0_8,
    CF_CALLBACK_PARAMETERS_0_9,
};

#[derive(Debug, Clone, Copy)]
pub struct FetchData(pub(crate) CF_CALLBACK_PARAMETERS_0_6);

impl FetchData {
    pub fn recovery(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_FETCH_DATA_FLAG_RECOVERY).0 != 0
    }

    pub fn explicit_hydration(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_FETCH_DATA_FLAG_EXPLICIT_HYDRATION).0 != 0
    }

    pub fn required_file_range(&self) -> Range<u64> {
        (self.0.RequiredFileOffset as u64)
            ..(self.0.RequiredFileOffset + self.0.RequiredLength) as u64
    }

    pub fn optional_file_range(&self) -> Range<u64> {
        (self.0.OptionalFileOffset as u64)
            ..(self.0.OptionalFileOffset + self.0.OptionalLength) as u64
    }

    pub fn last_dehydration_time(&self) -> u64 {
        self.0.LastDehydrationTime as u64
    }

    pub fn last_dehydration_reason(&self) -> DehydrationReason {
        self.0.LastDehydrationReason.into()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ValidateData(pub(crate) CF_CALLBACK_PARAMETERS_0_11);

impl ValidateData {
    pub fn explicit_hydration(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_VALIDATE_DATA_FLAG_EXPLICIT_HYDRATION).0 != 0
    }

    pub fn required_file_range(&self) -> Range<u64> {
        (self.0.RequiredFileOffset as u64)
            ..(self.0.RequiredFileOffset + self.0.RequiredLength) as u64
    }
}

// TODO: Does this take this parameter?
#[derive(Clone, Copy)]
pub struct Cancel(pub(crate) CF_CALLBACK_PARAMETERS_0_0);

impl Cancel {
    pub fn io_timeout(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_CANCEL_FLAG_IO_TIMEOUT).0 != 0
    }

    pub fn io_aborted(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_CANCEL_FLAG_IO_ABORTED).0 != 0
    }

    pub fn file_range(&self) -> Range<u64> {
        unsafe {
            (self.0.Anonymous.FetchData.FileOffset as u64)
                ..(self.0.Anonymous.FetchData.FileOffset + self.0.Anonymous.FetchData.Length) as u64
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FetchPlaceholders(pub(crate) CF_CALLBACK_PARAMETERS_0_7);

impl FetchPlaceholders {
    #[cfg(feature = "globs")]
    pub fn pattern(&self) -> Result<globset::Glob, globset::Error> {
        let pattern = unsafe { U16CStr::from_ptr_str(self.0.Pattern.0) }.to_string_lossy();
        globset::Glob::new(&pattern)
    }

    #[cfg(not(feature = "globs"))]
    pub fn pattern(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.0.Pattern.0) }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Opened(pub(crate) CF_CALLBACK_PARAMETERS_0_8);

impl Opened {
    pub fn placeholder_unknown(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_OPEN_COMPLETION_FLAG_PLACEHOLDER_UNKNOWN).0 != 0
    }

    pub fn placeholder_unsupported(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_OPEN_COMPLETION_FLAG_PLACEHOLDER_UNSUPPORTED).0
            != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Closed(pub(crate) CF_CALLBACK_PARAMETERS_0_1);

impl Closed {
    pub fn deleted(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_CLOSE_COMPLETION_FLAG_DELETED).0 != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Dehydrate(pub(crate) CF_CALLBACK_PARAMETERS_0_3);

impl Dehydrate {
    pub fn background(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_DEHYDRATE_FLAG_BACKGROUND).0 != 0
    }

    pub fn reason(&self) -> DehydrationReason {
        self.0.Reason.into()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Dehydrated(pub(crate) CF_CALLBACK_PARAMETERS_0_2);

impl Dehydrated {
    pub fn background(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_DEHYDRATE_COMPLETION_FLAG_BACKGROUND).0 != 0
    }

    pub fn dehydrated(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_DEHYDRATE_COMPLETION_FLAG_DEHYDRATED).0 != 0
    }

    pub fn reason(&self) -> DehydrationReason {
        self.0.Reason.into()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Delete(pub(crate) CF_CALLBACK_PARAMETERS_0_5);

impl Delete {
    pub fn is_directory(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_DELETE_FLAG_IS_DIRECTORY).0 != 0
    }

    pub fn is_undelete(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_DELETE_FLAG_IS_UNDELETE).0 != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Deleted(pub(crate) CF_CALLBACK_PARAMETERS_0_4);

#[derive(Debug, Clone, Copy)]
pub struct Rename(pub(crate) CF_CALLBACK_PARAMETERS_0_10);

impl Rename {
    pub fn is_directory(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_RENAME_FLAG_IS_DIRECTORY).0 != 0
    }

    pub fn source_in_scope(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_RENAME_FLAG_SOURCE_IN_SCOPE).0 != 0
    }

    pub fn target_in_scope(&self) -> bool {
        (self.0.Flags & CloudFilters::CF_CALLBACK_RENAME_FLAG_TARGET_IN_SCOPE).0 != 0
    }

    // TODO: all I could really do here is cache the value, I'd need to do the same for below, source_path
    pub fn target_path(&self) -> PathBuf {
        unsafe {
            U16CStr::from_ptr_str(self.0.TargetPath.0)
                .to_os_string()
                .into()
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Renamed(pub(crate) CF_CALLBACK_PARAMETERS_0_9);

impl Renamed {
    pub fn source_path(&self) -> PathBuf {
        unsafe {
            U16CStr::from_ptr_str(self.0.SourcePath.0)
                .to_os_string()
                .into()
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DehydrationReason {
    UserManual,
    SystemLowSpace,
    SystemInactivity,
    SystemOsUpgrade,
}

impl From<CF_CALLBACK_DEHYDRATION_REASON> for DehydrationReason {
    fn from(value: CF_CALLBACK_DEHYDRATION_REASON) -> Self {
        match value {
            CloudFilters::CF_CALLBACK_DEHYDRATION_REASON_USER_MANUAL => Self::UserManual,
            CloudFilters::CF_CALLBACK_DEHYDRATION_REASON_SYSTEM_LOW_SPACE => Self::SystemLowSpace,
            CloudFilters::CF_CALLBACK_DEHYDRATION_REASON_SYSTEM_INACTIVITY => {
                Self::SystemInactivity
            }
            CloudFilters::CF_CALLBACK_DEHYDRATION_REASON_SYSTEM_OS_UPGRADE => Self::SystemOsUpgrade,
            _ => unreachable!(),
        }
    }
}
