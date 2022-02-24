pub mod info;
pub mod proxy;
pub mod ticket;

use std::path::{Path, PathBuf};

use widestring::U16CString;

use crate::{com::SourceStatus, error::CloudErrorKind, request::Request};

pub trait SyncFilter: Send + Sync {
    /// Callback to satisfy an I/O request, or a placeholder hydration request.
    fn fetch_data(
        &self,
        _request: Request,
        _ticket: ticket::FetchData,
        _info: info::FetchData,
    ) -> Result<(), CloudErrorKind> {
        Err(CloudErrorKind::NotSupported)
    }

    /// Callback to cancel an ongoing placeholder hydration.
    fn cancel_fetch_data(&self, _request: Request, _info: info::CancelFetchData) {}

    /// Callback to validate placeholder data.
    fn validate_data(
        &self,
        _request: Request,
        _ticket: ticket::ValidateData,
        _info: info::ValidateData,
    ) -> Result<(), CloudErrorKind> {
        Err(CloudErrorKind::NotSupported)
    }

    /// Callback to request information about the contents of placeholder files.
    fn fetch_placeholders(
        &self,
        _request: Request,
        _ticket: ticket::FetchPlaceholders,
        _info: info::FetchPlaceholders,
    ) -> Result<(), CloudErrorKind> {
        Err(CloudErrorKind::NotSupported)
    }

    /// Callback to cancel a request for the contents of placeholder files.
    fn cancel_fetch_placeholders(&self, _request: Request, _info: info::CancelFetchPlaceholders) {}

    /// Callback to inform the sync provider that a placeholder under one of its
    /// sync roots has been successfully opened for read/write/delete access.
    fn opened(&self, _request: Request, _info: info::Opened) {}

    /// Callback to inform the sync provider that a placeholder under one of its
    /// sync roots that has been previously opened for read/write/delete access
    /// is now closed.
    fn closed(&self, _request: Request, _info: info::Closed) {}

    /// Callback to inform the sync provider that a placeholder under one of its
    /// sync roots is about to be dehydrated.
    fn dehydrate(
        &self,
        _request: Request,
        _ticket: ticket::Dehydrate,
        _info: info::Dehydrate,
    ) -> Result<(), CloudErrorKind> {
        Err(CloudErrorKind::NotSupported)
    }

    fn dehydrated(&self, _request: Request, _info: info::Dehydrated) {}

    /// Callback to inform the sync provider that a placeholder under one of its
    /// sync roots is about to be deleted.
    fn delete(
        &self,
        _request: Request,
        _ticket: ticket::Delete,
        _info: info::Delete,
    ) -> Result<(), CloudErrorKind> {
        Err(CloudErrorKind::NotSupported)
    }

    fn deleted(&self, _request: Request, _info: info::Deleted) {}

    /// Callback to inform the sync provider that a placeholder under one of its
    /// sync roots is about to be renamed or moved.
    fn rename(
        &self,
        _request: Request,
        _ticket: ticket::Rename,
        _info: info::Rename,
    ) -> Result<(), CloudErrorKind> {
        Err(CloudErrorKind::NotSupported)
    }

    fn renamed(&self, _request: Request, _info: info::Renamed) {}
}

pub trait SyncFilterExt: SyncFilter {
    // IThumbnailProvider - gets the file thumbnail for the specified path
    // this should return a bitmap and the alpha type; needs a builder-wrapper
    fn fetch_thumbnail(&self, _path: &Path, _size: u32) {}

    // IStorageProviderUriSource - gets the url for the specified path
    // GetContentInfoForPath - what is the diff between ContentId and ContentUri?
    // The uri is the id, just with a parameter specifying the sync provider?
    fn fetch_uri(&self, _path: PathBuf) -> Result<U16CString, SourceStatus> {
        Err(SourceStatus::FileNotFound)
    }

    // GetPathForContentUri
    // the error is a StorageProviderUriSourceStatus
    fn fetch_path(&self, _uri: U16CString) -> Result<PathBuf, SourceStatus> {
        Err(SourceStatus::FileNotFound)
    }

    // IStorageProviderPropertyCapabilities - should this be a method or should a
    // list of supported properties be provided on registration?
    // https://docs.microsoft.com/en-us/windows/win32/properties/props
    // this corresponds with this invoked method, https://docs.microsoft.com/en-us/uwp/api/windows.storage.storageprovider.ispropertysupportedforpartialfileasync?view=winrt-22000#windows-storage-storageprovider-ispropertysupportedforpartialfileasync(system-string)
    fn is_property_supported(&self, _property: U16CString) -> bool {
        false
    }

    // IStorageProviderItemPropertySource
    // https://docs.microsoft.com/en-us/uwp/api/windows.storage.provider.storageprovidersyncrootinfo?view=winrt-22000
    // must be specified on registration here, StorageProviderItemPropertyDefinitions
    // returns an array of the latter with corresponding values
    fn fetch_properties(&self, _path: PathBuf) -> Vec<()> {
        Vec::new()
    }
}
