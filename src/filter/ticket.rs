use std::ops::Range;

use windows::core;

use crate::{
    command::{self, Command, Fallible},
    error::CloudErrorKind,
    request::Keys,
};

#[derive(Debug, Clone, Copy)]
pub struct FetchData(pub(crate) Keys);

impl FetchData {
    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Write::fail(self.0, error_kind)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ValidateData(pub(crate) Keys);

impl ValidateData {
    // TODO: make this generic over a RangeBounds
    pub fn pass(&self, range: Range<u64>) -> core::Result<()> {
        command::Validate { range }.execute(self.0)
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Validate::fail(self.0, error_kind)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FetchPlaceholders(pub(crate) Keys);

impl FetchPlaceholders {
    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::CreatePlaceholders::fail(self.0, error_kind).and(Ok(()))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Dehydrate(pub(crate) Keys);

impl Dehydrate {
    pub fn pass(&self) -> core::Result<()> {
        command::Dehydrate { blob: None }.execute(self.0)
    }

    pub fn pass_with_blob(&self, blob: &[u8]) -> core::Result<()> {
        command::Dehydrate { blob: Some(blob) }.execute(self.0)
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Dehydrate::fail(self.0, error_kind)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Delete(pub(crate) Keys);

impl Delete {
    pub fn pass(&self) -> core::Result<()> {
        command::Delete.execute(self.0)
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Delete::fail(self.0, error_kind)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rename(pub(crate) Keys);

impl Rename {
    pub fn pass(&self) -> core::Result<()> {
        command::Rename.execute(self.0)
    }

    pub fn fail(&self, error_kind: CloudErrorKind) -> core::Result<()> {
        command::Rename::fail(self.0, error_kind)
    }
}
