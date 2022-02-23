use std::{path::PathBuf, slice};

use widestring::{U16CStr, U16CString};
use windows::{
    core,
    Win32::Storage::CloudFilters::{CF_CALLBACK_INFO, CF_PROCESS_INFO},
};

use crate::{
    command::{Command, CreatePlaceholders, Dehydrate, Delete, Fallible, Rename, Validate, Write},
    filter::CallbackType,
    logger::Reason,
    placeholder::Placeholder,
    placeholder_file::PlaceholderFile,
    provider::Provider,
};

#[derive(Debug, Clone, Copy)]
pub struct Request {
    info: CF_CALLBACK_INFO,
    kind: CallbackType,
}

impl Request {
    pub(crate) fn new(info: CF_CALLBACK_INFO, kind: CallbackType) -> Self {
        Self { info, kind }
    }

    pub fn connection_key(&self) -> isize {
        self.info.ConnectionKey.0
    }

    // TODO: Is this a volume guid path or its actual guid? If so, return a GUID
    // instance
    pub fn volume_guid_name(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.info.VolumeGuidName.0) }
    }

    pub fn volume_dos_name(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.info.VolumeDosName.0) }
    }

    pub fn volume_serial_number(&self) -> u32 {
        self.info.VolumeSerialNumber
    }

    pub fn sync_root_file_id(&self) -> i64 {
        self.info.SyncRootFileId
    }

    pub fn file_id(&self) -> i64 {
        self.info.FileId
    }

    pub fn file_size(&self) -> u64 {
        self.info.FileSize as u64
    }

    // TODO: Create a U16Path struct to avoid an extra allocation
    // For now this should be cached on creation
    pub fn path(&self) -> PathBuf {
        unsafe { U16CStr::from_ptr_str(self.info.NormalizedPath.0) }
            .to_os_string()
            .into()
    }

    pub fn transfer_key(&self) -> i64 {
        self.info.TransferKey
    }

    // ranges from 0-15 (CF_MAX_PRIORITY_HINT)
    pub fn priority_hint(&self) -> u8 {
        self.info.PriorityHint
    }

    // TODO: this is optional depending on whether they specified the flag on
    // connect?
    pub fn process(&self) -> Process {
        Process(unsafe { *self.info.ProcessInfo })
    }

    pub fn request_key(&self) -> i64 {
        self.info.RequestKey
    }

    pub fn file_blob(&self) -> &[u8] {
        match self.info.FileIdentityLength {
            0 => panic!("TODO"),
            _ => unsafe {
                slice::from_raw_parts(
                    self.info.FileIdentity as *mut u8,
                    self.info.FileIdentityLength as usize,
                )
            },
        }
    }

    pub fn register_blob(&self) -> &[u8] {
        match self.info.FileIdentityLength {
            0 => panic!("TODO"),
            _ => unsafe {
                slice::from_raw_parts(
                    self.info.SyncRootIdentity as *mut u8,
                    self.info.SyncRootIdentityLength as usize,
                )
            },
        }
    }

    pub fn placeholder(&self) -> Placeholder {
        Placeholder::new(self.keys(), self.path(), self.file_size())
    }

    pub fn provider(&self) -> Provider {
        Provider::new(self.connection_key())
    }

    // https://docs.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_callback_type#remarks
    // after 60 seconds of no report, windows will cancel the request with an error,
    // this function is a "hack" to avoid the timeout
    // https://docs.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfexecute#remarks
    // CfExecute will reset any timers as stated
    pub fn reset_timeout() {}

    pub fn create_placeholder(&self, placeholder: PlaceholderFile) -> core::Result<u32> {
        CreatePlaceholders {
            placeholders: &[placeholder],
        }
        .execute(self.keys(), None)
    }

    // TODO: change this method and the one above to return the errors and placeholders for each failed creation
    pub fn create_placeholders(&self, placeholders: &[PlaceholderFile]) -> core::Result<u32> {
        CreatePlaceholders { placeholders }.execute(self.keys(), None)
    }

    pub fn fail(&self) -> core::Result<()> {
        self._fail(None)
    }

    pub fn fail_with_reason(&self, reason: Reason) -> core::Result<()> {
        // TODO: pass the error to the logger
        self._fail(Some(reason))
    }

    pub(crate) fn keys(&self) -> Keys {
        Keys {
            connection_key: self.connection_key(),
            transfer_key: self.transfer_key(),
            request_key: self.request_key(),
        }
    }

    // call this to fail early
    // TODO: I'm thinking these methods should be moved to tickets since some can't actually fail
    fn _fail(&self, reason: Option<Reason>) -> core::Result<()> {
        macro_rules! fail {
            ($struct: ident) => {
                $struct::fail(self.keys(), reason)
            };
        }

        match self.kind {
            CallbackType::FetchData => fail!(Write),
            CallbackType::ValidateData => fail!(Validate),
            CallbackType::FetchPlaceholders => fail!(CreatePlaceholders).and(Ok(())),
            CallbackType::Dehydrate => fail!(Dehydrate),
            CallbackType::Delete => fail!(Delete),
            CallbackType::Rename => fail!(Rename),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Process(CF_PROCESS_INFO);

impl Process {
    pub fn name(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.0.PackageName.0) }
    }

    pub fn id(&self) -> u32 {
        self.0.ProcessId
    }

    // TODO: read command_line
    pub fn session_id(&self) -> u32 {
        self.0.SessionId
    }

    pub fn application_id(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.0.ApplicationId.0) }
    }

    // TODO: command_line and session_id are valid only in versions 1803+
    // https://docs.microsoft.com/en-us/windows/win32/api/cfapi/ns-cfapi-cf_process_info#sessionid
    pub fn command_line(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.0.CommandLine.0) }
    }

    // TODO: Could be optimized
    pub fn image_path(&self) -> Option<PathBuf> {
        let path = unsafe { U16CString::from_ptr_str(self.0.ImagePath.0) };
        if path == unsafe { U16CString::from_str_unchecked("UNKNOWN") } {
            None
        } else {
            Some(path.to_os_string().into())
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Keys {
    pub connection_key: isize,
    pub request_key: i64,
    pub transfer_key: i64,
}
