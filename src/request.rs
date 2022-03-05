use std::{path::PathBuf, slice};

use widestring::{U16CStr, U16CString};
use windows::{
    core,
    Win32::Storage::CloudFilters::{CF_CALLBACK_INFO, CF_PROCESS_INFO},
};

use crate::{
    command::{Command, CreatePlaceholders},
    placeholder::Placeholder,
    placeholder_file::PlaceholderFile,
    usn::Usn,
};

pub type RawConnectionKey = isize;
pub type RawTransferKey = i64;

#[derive(Debug)]
pub struct Request(CF_CALLBACK_INFO);

impl Request {
    pub(crate) fn new(info: CF_CALLBACK_INFO) -> Self {
        Self(info)
    }

    pub fn connection_key(&self) -> RawConnectionKey {
        self.0.ConnectionKey.0
    }

    pub fn transfer_key(&self) -> RawTransferKey {
        self.0.TransferKey
    }

    pub fn volume_guid_name(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.0.VolumeGuidName.0) }
    }

    pub fn volume_dos_name(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.0.VolumeDosName.0) }
    }

    pub fn volume_serial_number(&self) -> u32 {
        self.0.VolumeSerialNumber
    }

    pub fn process(&self) -> Process {
        Process(unsafe { *self.0.ProcessInfo })
    }

    pub fn sync_root_file_id(&self) -> i64 {
        self.0.SyncRootFileId
    }

    pub fn file_id(&self) -> i64 {
        self.0.FileId
    }

    pub fn file_size(&self) -> u64 {
        self.0.FileSize as u64
    }

    // TODO: Create a U16Path struct to avoid an extra allocation
    // For now this should be cached on creation
    pub fn path(&self) -> PathBuf {
        unsafe { U16CStr::from_ptr_str(self.0.NormalizedPath.0) }
            .to_os_string()
            .into()
    }

    // ranges from 0-CF_MAX_PRIORITY_HINT (15)
    pub fn priority_hint(&self) -> u8 {
        self.0.PriorityHint
    }

    // https://docs.microsoft.com/en-us/answers/questions/749979/what-is-a-requestkey-cfapi.html
    // pub fn request_key(&self) -> i64 {
    //     self.0.RequestKey
    // }

    // TODO: move file blob and file-related stuff to the placeholder struct?
    pub fn file_blob(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                self.0.FileIdentity as *mut u8,
                self.0.FileIdentityLength as usize,
            )
        }
    }

    pub fn register_blob(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                self.0.SyncRootIdentity as *mut u8,
                self.0.SyncRootIdentityLength as usize,
            )
        }
    }

    pub fn placeholder(&self) -> Placeholder {
        Placeholder::new(
            self.connection_key(),
            self.transfer_key(),
            self.path(),
            self.file_size(),
        )
    }

    // https://docs.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_callback_type#remarks
    // after 60 seconds of no report, windows will cancel the request with an error,
    // this function is a "hack" to avoid the timeout
    // https://docs.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfexecute#remarks
    // CfExecute will reset any timers as stated
    pub fn reset_timeout() {}

    #[inline]
    pub fn create_placeholder(&self, placeholder: PlaceholderFile) -> core::Result<Usn> {
        self.create_placeholders(&[placeholder])
            .map(|mut x| x.remove(0))?
    }

    #[inline]
    pub fn create_placeholders(
        &self,
        placeholders: &[PlaceholderFile],
    ) -> core::Result<Vec<core::Result<Usn>>> {
        self.create_placeholders_with_total(placeholders, placeholders.len() as u64)
    }

    pub fn create_placeholders_with_total(
        &self,
        placeholders: &[PlaceholderFile],
        total: u64,
    ) -> core::Result<Vec<core::Result<Usn>>> {
        CreatePlaceholders {
            placeholders,
            total,
        }
        .execute(self.connection_key(), self.transfer_key())
    }
}

#[derive(Debug)]
pub struct Process(CF_PROCESS_INFO);

impl Process {
    pub fn name(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.0.PackageName.0) }
    }

    pub fn id(&self) -> u32 {
        self.0.ProcessId
    }

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
