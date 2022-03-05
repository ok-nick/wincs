use std::ops::Range;

use windows::core;

use crate::{
    command::{self, Command, Fallible},
    error::CloudErrorKind,
    request::{RawConnectionKey, RawTransferKey},
};

#[derive(Debug)]
pub struct FetchData {
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
}

impl FetchData {
    pub fn new(connection_key: RawConnectionKey, transfer_key: RawTransferKey) -> Self {
        Self {
            connection_key,
            transfer_key,
        }
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Write::fail(self.connection_key, self.transfer_key, error_kind)
    }
}

#[derive(Debug)]
pub struct ValidateData {
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
}

impl ValidateData {
    pub fn new(connection_key: RawConnectionKey, transfer_key: RawTransferKey) -> Self {
        Self {
            connection_key,
            transfer_key,
        }
    }

    // TODO: make this generic over a RangeBounds
    // if the range specified is past the current file length, will it consider that range to be validated?
    // https://docs.microsoft.com/en-us/answers/questions/750302/if-the-ackdata-field-of-cf-operation-parameters-is.html
    pub fn pass(&self, range: Range<u64>) -> core::Result<()> {
        command::Validate { range }.execute(self.connection_key, self.transfer_key)
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Validate::fail(self.connection_key, self.transfer_key, error_kind)
    }
}

#[derive(Debug)]
pub struct FetchPlaceholders {
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
}

impl FetchPlaceholders {
    pub fn new(connection_key: RawConnectionKey, transfer_key: RawTransferKey) -> Self {
        Self {
            connection_key,
            transfer_key,
        }
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::CreatePlaceholders::fail(self.connection_key, self.transfer_key, error_kind)
            .and(Ok(()))
    }
}

#[derive(Debug)]
pub struct Dehydrate {
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
}

impl Dehydrate {
    pub fn new(connection_key: RawConnectionKey, transfer_key: RawTransferKey) -> Self {
        Self {
            connection_key,
            transfer_key,
        }
    }

    pub fn pass(&self) -> core::Result<()> {
        command::Dehydrate { blob: None }.execute(self.connection_key, self.transfer_key)
    }

    pub fn pass_with_blob(&self, blob: &[u8]) -> core::Result<()> {
        command::Dehydrate { blob: Some(blob) }.execute(self.connection_key, self.transfer_key)
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Dehydrate::fail(self.connection_key, self.transfer_key, error_kind)
    }
}

#[derive(Debug)]
pub struct Delete {
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
}

impl Delete {
    pub fn new(connection_key: RawConnectionKey, transfer_key: RawTransferKey) -> Self {
        Self {
            connection_key,
            transfer_key,
        }
    }

    pub fn pass(&self) -> core::Result<()> {
        command::Delete.execute(self.connection_key, self.transfer_key)
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Delete::fail(self.connection_key, self.transfer_key, error_kind)
    }
}

#[derive(Debug)]
pub struct Rename {
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
}

impl Rename {
    pub fn new(connection_key: RawConnectionKey, transfer_key: RawTransferKey) -> Self {
        Self {
            connection_key,
            transfer_key,
        }
    }

    pub fn pass(&self) -> core::Result<()> {
        command::Rename.execute(self.connection_key, self.transfer_key)
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Rename::fail(self.connection_key, self.transfer_key, error_kind)
    }
}
