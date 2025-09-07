use std::{path::PathBuf, slice};

use widestring::{U16CStr, U16CString};
use windows::Win32::Storage::CloudFilters::{CF_CALLBACK_INFO, CF_PROCESS_INFO};

use crate::placeholder::Placeholder;

pub type RawConnectionKey = i64;
pub type RawTransferKey = i64;

/// A struct containing various information for the current file operation.
///
/// If there is no activity on the placeholder (the methods in the
/// [Placeholder][crate::Placeholder] struct returned by
/// [Request::placeholder][crate::Request::placeholder]) within 60 seconds, the operating system
/// will automatically invalidate the request. To prevent this, read
/// [Request::reset_timeout][crate::Request::reset_timeout].
#[derive(Debug)]
pub struct Request(CF_CALLBACK_INFO);

impl Request {
    pub(crate) fn new(info: CF_CALLBACK_INFO) -> Self {
        Self(info)
    }

    /// A raw connection key used to identify the connection.
    pub fn connection_key(&self) -> RawConnectionKey {
        self.0.ConnectionKey.0
    }

    /// A raw transfer key used to identify the current file operation.
    pub fn transfer_key(&self) -> RawTransferKey {
        self.0.TransferKey
    }

    /// The GUID path of the current volume.
    ///
    /// The returned value comes in the form `\?\Volume{GUID}`.
    pub fn volume_guid_path(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.0.VolumeGuidName.0) }
    }

    /// The letter of the current volume.
    ///
    /// The returned value comes in the form `X:`, where `X` is the drive letter.
    pub fn volume_letter(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.0.VolumeDosName.0) }
    }

    /// The serial number of the current volume.
    pub fn volume_serial_number(&self) -> u32 {
        self.0.VolumeSerialNumber
    }

    /// Information of the user process that triggered the callback.
    pub fn process(&self) -> Process {
        Process(unsafe { *self.0.ProcessInfo })
    }

    /// The NTFS file ID of the sync root folder under which the placeholder being operated on
    /// resides.
    pub fn sync_root_file_id(&self) -> i64 {
        self.0.SyncRootFileId
    }

    /// The NTFS file ID of the placeholder file/directory.
    pub fn file_id(&self) -> i64 {
        self.0.FileId
    }

    /// The logical size of the placeholder file.
    ///
    /// If the placeholder is a directory, this value will always equal 0.
    pub fn file_size(&self) -> u64 {
        self.0.FileSize as u64
    }

    // TODO: Create a U16Path struct to avoid an extra allocation
    // For now this should be cached on creation
    /// The absolute path of the placeholder file/directory starting from the root directory of the
    /// volume.
    ///
    /// [Read here for more information on this
    /// function.](https://docs.microsoft.com/en-us/windows/win32/api/cfapi/ns-cfapi-cf_callback_info#remarks)
    pub fn path(&self) -> PathBuf {
        let mut path =
            PathBuf::from(unsafe { U16CStr::from_ptr_str(self.0.VolumeDosName.0) }.to_os_string());
        path.push(unsafe { U16CStr::from_ptr_str(self.0.NormalizedPath.0) }.to_os_string());

        path
    }

    /// A numeric scale ranging from
    /// 0-[15](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Storage/CloudFilters/constant.CF_MAX_PRIORITY_HINT.html)
    /// to describe the priority of the file operation.
    ///
    /// [Currently, this value does not
    /// change.](https://docs.microsoft.com/en-us/answers/questions/798674/priority-in-cf-callback-info.html)
    pub fn priority_hint(&self) -> u8 {
        self.0.PriorityHint
    }

    // https://docs.microsoft.com/en-us/answers/questions/749979/what-is-a-requestkey-cfapi.html
    // pub fn request_key(&self) -> i64 {
    //     self.0.RequestKey
    // }

    // TODO: move file blob and file-related stuff to the placeholder struct?
    /// The byte slice assigned to the current placeholder file/directory.
    pub fn file_blob(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                self.0.FileIdentity as *mut u8,
                self.0.FileIdentityLength as usize,
            )
        }
    }

    /// The byte slice assigned to the current sync root on registration.
    pub fn register_blob(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(
                self.0.SyncRootIdentity as *mut u8,
                self.0.SyncRootIdentityLength as usize,
            )
        }
    }

    /// Creates a new [Placeholder][crate::Placeholder] struct to perform various operations on the
    /// current placeholder file/directory.
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
    /// By default, the operating system will invalidate the callback after 60 seconds of no
    /// activity (meaning, no placeholder methods are invoked). If you are prone to this issue,
    /// consider calling this method or call placeholder methods more frequently.
    pub fn reset_timeout() {}
}

/// Information about the calling process.
#[derive(Debug)]
pub struct Process(CF_PROCESS_INFO);

impl Process {
    /// The application's package name.
    pub fn name(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.0.PackageName.0) }
    }

    /// The ID of the user process.
    pub fn id(&self) -> u32 {
        self.0.ProcessId
    }

    /// The ID of the session where the user process resides.
    pub fn session_id(&self) -> u32 {
        self.0.SessionId
    }

    /// The application's ID.
    pub fn application_id(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.0.ApplicationId.0) }
    }

    // TODO: command_line and session_id are valid only in versions 1803+
    // https://docs.microsoft.com/en-us/windows/win32/api/cfapi/ns-cfapi-cf_process_infoessionid
    /// The exact command used to initialize the user process.
    pub fn command_line(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr_str(self.0.CommandLine.0) }
    }

    // TODO: Could be optimized
    /// The absolute path to the main executable file of the process in the format of an NT path.
    ///
    /// This function returns [None][std::option::Option::None] when the operating system failed to
    /// retrieve the path.
    pub fn path(&self) -> Option<PathBuf> {
        let path = unsafe { U16CString::from_ptr_str(self.0.ImagePath.0) };
        if path == unsafe { U16CString::from_str_unchecked("UNKNOWN") } {
            None
        } else {
            Some(path.to_os_string().into())
        }
    }
}
