use std::ops::Range;

use windows::core;

use crate::{
    command::{self, Command, Fallible},
    error::CloudErrorKind,
    request::{RawConnectionKey, RawTransferKey},
    PlaceholderFile, Usn,
};

/// A ticket for the [SyncFilter::fetch_data][crate::SyncFilter::fetch_data] callback.
#[derive(Debug)]
pub struct FetchData {
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
}

impl FetchData {
    /// Create a new [FetchData][crate::ticket::FetchData].
    pub fn new(connection_key: RawConnectionKey, transfer_key: RawTransferKey) -> Self {
        Self {
            connection_key,
            transfer_key,
        }
    }

    /// Fail the callback with the specified error.
    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Write::fail(self.connection_key, self.transfer_key, error_kind)
    }
}

/// A ticket for the [SyncFilter::validate_data][crate::SyncFilter::validate_data] callback.
#[derive(Debug)]
pub struct ValidateData {
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
}

impl ValidateData {
    /// Create a new [ValidateData][crate::ticket::ValidateData].
    pub fn new(connection_key: RawConnectionKey, transfer_key: RawTransferKey) -> Self {
        Self {
            connection_key,
            transfer_key,
        }
    }

    // TODO: make this generic over a RangeBounds
    // if the range specified is past the current file length, will it consider that range to be validated?
    // https://docs.microsoft.com/en-us/answers/questions/750302/if-the-ackdata-field-of-cf-operation-parameters-is.html
    /// Confirms the specified range in the file is valid.
    pub fn pass(&self, range: Range<u64>) -> core::Result<()> {
        command::Validate { range }.execute(self.connection_key, self.transfer_key)
    }

    /// Fail the callback with the specified error.
    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Validate::fail(self.connection_key, self.transfer_key, error_kind)
    }
}

/// A ticket for the [SyncFilter::fetch_placeholders][crate::SyncFilter::fetch_placeholders] callback.
#[derive(Debug)]
pub struct FetchPlaceholders {
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
}

impl FetchPlaceholders {
    /// Create a new [FetchPlaceholders][crate::ticket::FetchPlaceholders].
    pub fn new(connection_key: RawConnectionKey, transfer_key: RawTransferKey) -> Self {
        Self {
            connection_key,
            transfer_key,
        }
    }

    /// Creates a list of placeholder files/directorys on the file system.
    ///
    /// The value returned is the final [Usn][crate::Usn] (and if they succeeded) after each placeholder is created.
    pub fn pass_with_placeholder(
        &self,
        placeholders: &mut [PlaceholderFile],
    ) -> core::Result<Vec<core::Result<Usn>>> {
        command::CreatePlaceholders {
            total: placeholders.len() as _,
            placeholders,
        }
        .execute(self.connection_key, self.transfer_key)
    }

    /// Fail the callback with the specified error.
    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::CreatePlaceholders::fail(self.connection_key, self.transfer_key, error_kind)
            .and(Ok(()))
    }
}

/// A ticket for the [SyncFilter::dehydrate][crate::SyncFilter::dehydrate] callback.
#[derive(Debug)]
pub struct Dehydrate {
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
}

impl Dehydrate {
    /// Create a new [Dehydrate][crate::ticket::Dehydrate].
    pub fn new(connection_key: RawConnectionKey, transfer_key: RawTransferKey) -> Self {
        Self {
            connection_key,
            transfer_key,
        }
    }

    /// Confirms dehydration of the file.
    pub fn pass(&self) -> core::Result<()> {
        command::Dehydrate { blob: None }.execute(self.connection_key, self.transfer_key)
    }

    /// Confirms dehydration of the file and updates its file blob.
    pub fn pass_with_blob(&self, blob: &[u8]) -> core::Result<()> {
        command::Dehydrate { blob: Some(blob) }.execute(self.connection_key, self.transfer_key)
    }

    /// Fail the callback with the specified error.
    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Dehydrate::fail(self.connection_key, self.transfer_key, error_kind)
    }
}

/// A ticket for the [SyncFilter::delete][crate::SyncFilter::delete] callback.
#[derive(Debug)]
pub struct Delete {
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
}

impl Delete {
    /// Create a new [Delete][crate::ticket::Delete].
    pub fn new(connection_key: RawConnectionKey, transfer_key: RawTransferKey) -> Self {
        Self {
            connection_key,
            transfer_key,
        }
    }

    /// Confirms deletion of the file.
    pub fn pass(&self) -> core::Result<()> {
        command::Delete.execute(self.connection_key, self.transfer_key)
    }

    /// Fail the callback with the specified error.
    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Delete::fail(self.connection_key, self.transfer_key, error_kind)
    }
}

/// A ticket for the [SyncFilter::rename][crate::SyncFilter::rename] callback.
#[derive(Debug)]
pub struct Rename {
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
}

impl Rename {
    /// Create a new [Rename][crate::ticket::Rename].
    pub fn new(connection_key: RawConnectionKey, transfer_key: RawTransferKey) -> Self {
        Self {
            connection_key,
            transfer_key,
        }
    }

    /// Confirms the rename/move of a file.
    pub fn pass(&self) -> core::Result<()> {
        command::Rename.execute(self.connection_key, self.transfer_key)
    }

    /// Fail the callback with the specified error.
    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Rename::fail(self.connection_key, self.transfer_key, error_kind)
    }
}
