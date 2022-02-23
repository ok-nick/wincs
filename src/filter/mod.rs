pub mod info;
pub mod proxy;
pub mod ticket;

use std::path::{Path, PathBuf};

use widestring::U16CString;
use windows::Win32::Storage::CloudFilters::{self, CF_CALLBACK_TYPE};

use crate::{com::source::SourceStatus, logger::ErrorReason, request::Request};

// https://docs.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_callback_type
// TODO: rather than returning Ok, return an Unsupported reason?
pub trait SyncFilter: Send + Sync {
    type Error: ErrorReason;

    /// Callback to satisfy an I/O request, or a placeholder hydration request.
    fn fetch_data(&self, _request: Request, _info: info::FetchData) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Callback to cancel an ongoing placeholder hydration.
    fn cancel_fetch_data(&self, _request: Request, _info: info::Cancel) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Callback to validate placeholder data.
    fn validate_data(
        &self,
        _request: Request,
        _ticket: ticket::ValidateData,
        _info: info::ValidateData,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Callback to request information about the contents of placeholder files.
    fn fetch_placeholders(
        &self,
        _request: Request,
        _info: info::FetchPlaceholders,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Callback to cancel a request for the contents of placeholder files.
    fn cancel_fetch_placeholders(
        &self,
        _request: Request,
        _info: info::Cancel,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Callback to inform the sync provider that a placeholder under one of its
    /// sync roots has been successfully opened for read/write/delete access.
    fn opened(&self, _request: Request, _info: info::Opened) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Callback to inform the sync provider that a placeholder under one of its
    /// sync roots that has been previously opened for read/write/delete access
    /// is now closed.
    fn closed(&self, _request: Request, _info: info::Closed) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Callback to inform the sync provider that a placeholder under one of its
    /// sync roots is about to be dehydrated.
    fn dehydrate(
        &self,
        _request: Request,
        _ticket: ticket::Dehydrate,
        _info: info::Dehydrate,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn dehydrated(&self, _request: Request, _info: info::Dehydrated) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Callback to inform the sync provider that a placeholder under one of its
    /// sync roots is about to be deleted.
    fn delete(
        &self,
        _request: Request,
        _ticket: ticket::Delete,
        _info: info::Delete,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn deleted(&self, _request: Request, _info: info::Deleted) -> Result<(), Self::Error> {
        Ok(())
    }

    /// Callback to inform the sync provider that a placeholder under one of its
    /// sync roots is about to be renamed or moved.
    fn rename(
        &self,
        _request: Request,
        _ticket: ticket::Rename,
        _info: info::Rename,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn renamed(&self, _request: Request, _info: info::Renamed) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub trait SyncFilterExt: SyncFilter {
    // IThumbnailProvider - gets the file thumbnail for the specified path
    // this should return a bitmap and the alpha type; needs a builder-wrapper
    fn fetch_thumbnail(&self, _path: &Path, _size: u32) -> Result<(), Self::Error> {
        Ok(())
    }

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

    // IStorageProviderStatusSource - returns a bunch of status information for
    // files and the sync root.
    // TODO: this should probably be handled internally and combined w/ the win32
    // funcs
    fn status(&self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
pub enum CallbackType {
    FetchData,
    CancelFetchData,
    ValidateData,
    FetchPlaceholders,
    CancelFetchPlaceholders,
    Opened,
    Closed,
    Dehydrate,
    Dehydrated,
    Delete,
    Deleted,
    Rename,
    Renamed,
}

impl From<CF_CALLBACK_TYPE> for CallbackType {
    fn from(callback_type: CF_CALLBACK_TYPE) -> Self {
        match callback_type {
            CloudFilters::CF_CALLBACK_TYPE_FETCH_DATA => CallbackType::FetchData,
            CloudFilters::CF_CALLBACK_TYPE_CANCEL_FETCH_DATA => CallbackType::CancelFetchData,
            CloudFilters::CF_CALLBACK_TYPE_VALIDATE_DATA => CallbackType::ValidateData,
            CloudFilters::CF_CALLBACK_TYPE_FETCH_PLACEHOLDERS => CallbackType::FetchPlaceholders,
            CloudFilters::CF_CALLBACK_TYPE_CANCEL_FETCH_PLACEHOLDERS => {
                CallbackType::CancelFetchPlaceholders
            }
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_FILE_OPEN_COMPLETION => CallbackType::Opened,
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_FILE_CLOSE_COMPLETION => CallbackType::Closed,
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_DEHYDRATE => CallbackType::Dehydrate,
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_DEHYDRATE_COMPLETION => CallbackType::Dehydrated,
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_RENAME => CallbackType::Rename,
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_RENAME_COMPLETION => CallbackType::Renamed,
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_DELETE => CallbackType::Delete,
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_DELETE_COMPLETION => CallbackType::Deleted,
            _ => unreachable!(),
        }
    }
}

impl ToString for CallbackType {
    fn to_string(&self) -> String {
        match self {
            Self::FetchData => "fetch_data",
            Self::CancelFetchData => "cancel_fetch_data",
            Self::ValidateData => "validate_data",
            Self::FetchPlaceholders => "fetch_placeholders",
            Self::CancelFetchPlaceholders => "cancel_fetch_placeholders",
            Self::Opened => "opened",
            Self::Closed => "closed",
            Self::Dehydrate => "dehydrate",
            Self::Dehydrated => "dehydrated",
            Self::Delete => "delete",
            Self::Deleted => "deleted",
            Self::Rename => "rename",
            Self::Renamed => "renamed",
        }
        .to_owned()
    }
}
