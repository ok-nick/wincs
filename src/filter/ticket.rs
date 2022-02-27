use std::ops::Range;

use windows::core;

use crate::{
    command::{self, Command, Fallible},
    error::CloudErrorKind,
    key::{BorrowedConnectionKey, BorrowedTransferKey},
};

#[derive(Debug)]
pub struct FetchData<'a> {
    connection_key: &'a BorrowedConnectionKey,
    transfer_key: &'a BorrowedTransferKey,
}

impl<'a> FetchData<'a> {
    pub fn new<CK, TK>(connection_key: &'a CK, transfer_key: &'a TK) -> Self
    where
        CK: AsRef<BorrowedConnectionKey>,
        TK: AsRef<BorrowedTransferKey>,
    {
        Self {
            connection_key: connection_key.as_ref(),
            transfer_key: transfer_key.as_ref(),
        }
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Write::fail(
            *self.connection_key.key(),
            *self.transfer_key.key(),
            error_kind,
        )
    }
}

#[derive(Debug)]
pub struct ValidateData<'a> {
    connection_key: &'a BorrowedConnectionKey,
    transfer_key: &'a BorrowedTransferKey,
}

impl<'a> ValidateData<'a> {
    pub fn new<CK, TK>(connection_key: &'a CK, transfer_key: &'a TK) -> Self
    where
        CK: AsRef<BorrowedConnectionKey>,
        TK: AsRef<BorrowedTransferKey>,
    {
        Self {
            connection_key: connection_key.as_ref(),
            transfer_key: transfer_key.as_ref(),
        }
    }

    // TODO: make this generic over a RangeBounds
    // if the range specified is past the current file length, will it consider that range to be validated?
    // https://docs.microsoft.com/en-us/answers/questions/750302/if-the-ackdata-field-of-cf-operation-parameters-is.html
    pub fn pass(&self, range: Range<u64>) -> core::Result<()> {
        command::Validate { range }.execute(*self.connection_key.key(), *self.transfer_key.key())
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Validate::fail(
            *self.connection_key.key(),
            *self.transfer_key.key(),
            error_kind,
        )
    }
}

#[derive(Debug)]
pub struct FetchPlaceholders<'a> {
    connection_key: &'a BorrowedConnectionKey,
    transfer_key: &'a BorrowedTransferKey,
}

impl<'a> FetchPlaceholders<'a> {
    pub fn new<CK, TK>(connection_key: &'a CK, transfer_key: &'a TK) -> Self
    where
        CK: AsRef<BorrowedConnectionKey>,
        TK: AsRef<BorrowedTransferKey>,
    {
        Self {
            connection_key: connection_key.as_ref(),
            transfer_key: transfer_key.as_ref(),
        }
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::CreatePlaceholders::fail(
            *self.connection_key.key(),
            *self.transfer_key.key(),
            error_kind,
        )
        .and(Ok(()))
    }
}

#[derive(Debug)]
pub struct Dehydrate<'a> {
    connection_key: &'a BorrowedConnectionKey,
    transfer_key: &'a BorrowedTransferKey,
}

impl<'a> Dehydrate<'a> {
    pub fn new<CK, TK>(connection_key: &'a CK, transfer_key: &'a TK) -> Self
    where
        CK: AsRef<BorrowedConnectionKey>,
        TK: AsRef<BorrowedTransferKey>,
    {
        Self {
            connection_key: connection_key.as_ref(),
            transfer_key: transfer_key.as_ref(),
        }
    }

    pub fn pass(&self) -> core::Result<()> {
        command::Dehydrate { blob: None }
            .execute(*self.connection_key.key(), *self.transfer_key.key())
    }

    pub fn pass_with_blob(&self, blob: &[u8]) -> core::Result<()> {
        command::Dehydrate { blob: Some(blob) }
            .execute(*self.connection_key.key(), *self.transfer_key.key())
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Dehydrate::fail(
            *self.connection_key.key(),
            *self.transfer_key.key(),
            error_kind,
        )
    }
}

#[derive(Debug)]
pub struct Delete<'a> {
    connection_key: &'a BorrowedConnectionKey,
    transfer_key: &'a BorrowedTransferKey,
}

impl<'a> Delete<'a> {
    pub fn new<CK, TK>(connection_key: &'a CK, transfer_key: &'a TK) -> Self
    where
        CK: AsRef<BorrowedConnectionKey>,
        TK: AsRef<BorrowedTransferKey>,
    {
        Self {
            connection_key: connection_key.as_ref(),
            transfer_key: transfer_key.as_ref(),
        }
    }

    pub fn pass(&self) -> core::Result<()> {
        command::Delete.execute(*self.connection_key.key(), *self.transfer_key.key())
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Delete::fail(
            *self.connection_key.key(),
            *self.transfer_key.key(),
            error_kind,
        )
    }
}

#[derive(Debug)]
pub struct Rename<'a> {
    connection_key: &'a BorrowedConnectionKey,
    transfer_key: &'a BorrowedTransferKey,
}

impl<'a> Rename<'a> {
    pub fn new<CK, TK>(connection_key: &'a CK, transfer_key: &'a TK) -> Self
    where
        CK: AsRef<BorrowedConnectionKey>,
        TK: AsRef<BorrowedTransferKey>,
    {
        Self {
            connection_key: connection_key.as_ref(),
            transfer_key: transfer_key.as_ref(),
        }
    }

    pub fn pass(&self) -> core::Result<()> {
        command::Rename.execute(*self.connection_key.key(), *self.transfer_key.key())
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Rename::fail(
            *self.connection_key.key(),
            *self.transfer_key.key(),
            error_kind,
        )
    }
}
