pub mod basic;
pub mod printer;

use widestring::{U16CStr, U16CString, U16Str, U16String};
use windows::{
    core,
    Foundation::Uri,
    Storage::Provider::{StorageProviderError, StorageProviderErrorCommand, StorageProviderState},
    Win32::Foundation::{self, NTSTATUS},
};

use crate::root::hstring_from_widestring;

// TODO: this is basically like re-implementing the reporting methods in the Provider struct, somehow combine them
pub trait Logger {
    fn logs(&self) -> &[Reason];

    fn add_log(&mut self, reason: Reason);

    fn message(&self) -> &U16CStr;

    fn set_message(&mut self, message: U16String);

    fn state(&self) -> ProviderState;

    fn set_state(&mut self, state: ProviderState);
}

pub trait ErrorReason {
    fn code(&self) -> u32;

    fn title(&self) -> &U16Str;

    fn message(&self) -> &U16CStr;

    fn kind(&self) -> CloudErrorKind {
        CloudErrorKind::Unsuccessful
    }

    fn info(&self) -> Option<&Details> {
        None
    }

    fn primary_action(&self) -> Option<&Details> {
        None
    }

    fn secondary_action(&self) -> Option<&Details> {
        None
    }
}

impl<T: ErrorReason + 'static> From<T> for Box<dyn ErrorReason> {
    fn from(error: T) -> Self {
        Box::new(error)
    }
}

impl<T: ErrorReason> ErrorReason for Box<T> {
    fn code(&self) -> u32 {
        ErrorReason::code(&**self)
    }

    fn title(&self) -> &U16Str {
        ErrorReason::title(&**self)
    }

    fn message(&self) -> &U16CStr {
        ErrorReason::message(&**self)
    }

    fn info(&self) -> Option<&Details> {
        ErrorReason::info(&**self)
    }

    fn primary_action(&self) -> Option<&Details> {
        ErrorReason::primary_action(&**self)
    }

    fn secondary_action(&self) -> Option<&Details> {
        ErrorReason::secondary_action(&**self)
    }
}

#[derive(Debug, Clone)]
pub struct ReasonBuilder {
    // TODO: code is used as the Id under the hood
    code: u32,
    // TODO: this could be a ref?
    title: U16String,
    message: U16CString,
    info: Option<Details>,
    primary_action: Option<Details>,
    secondary_action: Option<Details>,
}

impl ReasonBuilder {
    pub fn new(code: u32, title: U16String, message: U16CString) -> Self {
        Self {
            code,
            message,
            title,
            info: None,
            primary_action: None,
            secondary_action: None,
        }
    }

    pub fn info(&mut self, details: Details) -> &mut Self {
        self.info = Some(details);
        self
    }

    pub fn primary_action(&mut self, details: Details) -> &mut Self {
        self.primary_action = Some(details);
        self
    }

    pub fn secondary_action(&mut self, details: Details) -> &mut Self {
        self.secondary_action = Some(details);
        self
    }

    pub fn build(self) -> Reason {
        Reason {
            code: self.code,
            message: self.message,
            title: self.title,
            info: self.info,
            primary_action: self.primary_action,
            secondary_action: self.secondary_action,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Reason {
    code: u32,
    title: U16String,
    message: U16CString,
    info: Option<Details>,
    primary_action: Option<Details>,
    secondary_action: Option<Details>,
}

impl ErrorReason for Reason {
    fn code(&self) -> u32 {
        self.code
    }

    fn title(&self) -> &U16Str {
        &self.title
    }

    fn message(&self) -> &U16CStr {
        &self.message
    }

    fn info(&self) -> Option<&Details> {
        self.info.as_ref()
    }

    fn primary_action(&self) -> Option<&Details> {
        self.primary_action.as_ref()
    }

    fn secondary_action(&self) -> Option<&Details> {
        self.secondary_action.as_ref()
    }
}

impl TryFrom<Reason> for StorageProviderError {
    type Error = core::Error;

    fn try_from(reason: Reason) -> Result<Self, Self::Error> {
        let error = StorageProviderError::CreateInstance(
            hstring_from_widestring(&U16String::from_str(&reason.code().to_string())),
            hstring_from_widestring(reason.title()),
            hstring_from_widestring(reason.message()),
        )?;
        if let Some(info) = reason.info {
            error.SetInformationalLink(StorageProviderErrorCommand::try_from(info)?)?;
        }
        if let Some(action) = reason.primary_action {
            error.SetPrimaryAction(StorageProviderErrorCommand::try_from(action)?)?;
        }
        if let Some(action) = reason.secondary_action {
            error.SetSecondaryAction(StorageProviderErrorCommand::try_from(action)?)?;
        }

        Ok(error)
    }
}

// TODO: could be refs?
#[derive(Debug, Clone)]
pub struct Details {
    // TODO: use a crate to represent uris?
    uri: U16String,
    label: U16String,
}

impl Details {
    pub fn new(uri: U16String, label: U16String) -> Self {
        Self { uri, label }
    }

    pub fn uri(&self) -> &U16Str {
        &self.uri
    }

    pub fn label(&self) -> &U16Str {
        &self.label
    }
}

impl TryFrom<Details> for StorageProviderErrorCommand {
    type Error = core::Error;

    fn try_from(details: Details) -> Result<Self, Self::Error> {
        StorageProviderErrorCommand::CreateInstance(
            hstring_from_widestring(details.uri()),
            Uri::CreateUri(hstring_from_widestring(details.label()))?,
        )
    }
}

// https://docs.microsoft.com/en-us/uwp/api/windows.storage.provider.storageproviderstate?view=winrt-22000
#[derive(Debug, Copy, Clone)]
pub enum ProviderState {
    Error,
    InSync,
    Offline,
    Paused,
    Syncing,
    Warning,
}

impl From<ProviderState> for StorageProviderState {
    fn from(state: ProviderState) -> StorageProviderState {
        match state {
            ProviderState::Error => StorageProviderState::Error,
            ProviderState::InSync => StorageProviderState::InSync,
            ProviderState::Offline => StorageProviderState::Offline,
            ProviderState::Paused => StorageProviderState::Paused,
            ProviderState::Syncing => StorageProviderState::Syncing,
            ProviderState::Warning => StorageProviderState::Warning,
        }
    }
}

// TODO: implement ToString
// I believe all of these types will provide different messages to the user, according to a microsoft employee at least
#[derive(Debug, Clone, Copy)]
pub enum CloudErrorKind {
    AccessDenied,
    AlreadyConnected,
    AuthenticationFailed,
    ConnectedProviderOnly,
    DehydrationDisallowed,
    IncompatibleHardlinks,
    InsufficientResources,
    InvalidRequest,
    InUse,
    MetadataCorrupt,
    MetadataTooLarge,
    NetworkUnavailable,
    NotInSync,
    NotSupported,
    NotUnderSyncRoot,
    Pinned,
    PropertyBlobChecksumMismatch,
    PropertyBlobTooLarge,
    PropertyCorrupt,
    PropertyLockConflict,
    PropertyVersionNotSupported,
    ProviderNotRunning,
    ProviderTerminated,
    ReadOnlyVolume,
    RequestAborted,
    RequestCancelled,
    RequestTimeout,
    SyncRootMetadataCorrupt,
    TooManyPropertyBlobs,
    Unsuccessful,
    ValidationFailed,
}

impl From<CloudErrorKind> for NTSTATUS {
    fn from(error: CloudErrorKind) -> Self {
        match error {
            CloudErrorKind::AccessDenied => Foundation::STATUS_CLOUD_FILE_ACCESS_DENIED,
            CloudErrorKind::AlreadyConnected => Foundation::STATUS_CLOUD_FILE_ALREADY_CONNECTED,
            CloudErrorKind::AuthenticationFailed => {
                Foundation::STATUS_CLOUD_FILE_AUTHENTICATION_FAILED
            }
            CloudErrorKind::ConnectedProviderOnly => {
                Foundation::STATUS_CLOUD_FILE_CONNECTED_PROVIDER_ONLY
            }
            CloudErrorKind::DehydrationDisallowed => {
                Foundation::STATUS_CLOUD_FILE_DEHYDRATION_DISALLOWED
            }
            CloudErrorKind::IncompatibleHardlinks => {
                Foundation::STATUS_CLOUD_FILE_INCOMPATIBLE_HARDLINKS
            }
            CloudErrorKind::InsufficientResources => {
                Foundation::STATUS_CLOUD_FILE_INSUFFICIENT_RESOURCES
            }
            CloudErrorKind::InvalidRequest => Foundation::STATUS_CLOUD_FILE_INVALID_REQUEST,
            CloudErrorKind::InUse => Foundation::STATUS_CLOUD_FILE_IN_USE,
            CloudErrorKind::MetadataCorrupt => Foundation::STATUS_CLOUD_FILE_METADATA_CORRUPT,
            CloudErrorKind::MetadataTooLarge => Foundation::STATUS_CLOUD_FILE_METADATA_TOO_LARGE,
            CloudErrorKind::NetworkUnavailable => Foundation::STATUS_CLOUD_FILE_NETWORK_UNAVAILABLE,
            CloudErrorKind::NotInSync => Foundation::STATUS_CLOUD_FILE_NOT_IN_SYNC,
            CloudErrorKind::NotSupported => Foundation::STATUS_CLOUD_FILE_NOT_SUPPORTED,
            CloudErrorKind::NotUnderSyncRoot => Foundation::STATUS_CLOUD_FILE_NOT_UNDER_SYNC_ROOT,
            CloudErrorKind::Pinned => Foundation::STATUS_CLOUD_FILE_PINNED,
            CloudErrorKind::PropertyBlobChecksumMismatch => {
                Foundation::STATUS_CLOUD_FILE_PROPERTY_BLOB_CHECKSUM_MISMATCH
            }
            CloudErrorKind::PropertyBlobTooLarge => {
                Foundation::STATUS_CLOUD_FILE_PROPERTY_BLOB_TOO_LARGE
            }
            CloudErrorKind::PropertyCorrupt => Foundation::STATUS_CLOUD_FILE_PROPERTY_CORRUPT,
            CloudErrorKind::PropertyLockConflict => {
                Foundation::STATUS_CLOUD_FILE_PROPERTY_LOCK_CONFLICT
            }
            CloudErrorKind::PropertyVersionNotSupported => {
                Foundation::STATUS_CLOUD_FILE_PROPERTY_VERSION_NOT_SUPPORTED
            }
            CloudErrorKind::ProviderNotRunning => {
                Foundation::STATUS_CLOUD_FILE_PROVIDER_NOT_RUNNING
            }
            CloudErrorKind::ProviderTerminated => Foundation::STATUS_CLOUD_FILE_PROVIDER_TERMINATED,
            CloudErrorKind::ReadOnlyVolume => Foundation::STATUS_CLOUD_FILE_READ_ONLY_VOLUME,
            CloudErrorKind::RequestAborted => Foundation::STATUS_CLOUD_FILE_REQUEST_ABORTED,
            CloudErrorKind::RequestCancelled => Foundation::STATUS_CLOUD_FILE_REQUEST_CANCELED,
            CloudErrorKind::RequestTimeout => Foundation::STATUS_CLOUD_FILE_REQUEST_TIMEOUT,
            CloudErrorKind::SyncRootMetadataCorrupt => {
                Foundation::STATUS_CLOUD_FILE_SYNC_ROOT_METADATA_CORRUPT
            }
            CloudErrorKind::TooManyPropertyBlobs => {
                Foundation::STATUS_CLOUD_FILE_TOO_MANY_PROPERTY_BLOBS
            }
            CloudErrorKind::Unsuccessful => Foundation::STATUS_CLOUD_FILE_UNSUCCESSFUL,
            CloudErrorKind::ValidationFailed => Foundation::STATUS_CLOUD_FILE_VALIDATION_FAILED,
        }
    }
}
