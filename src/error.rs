use windows::Win32::Foundation::{self, NTSTATUS};

// TODO: implement ToString
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
