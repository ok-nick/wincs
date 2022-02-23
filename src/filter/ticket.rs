use std::ops::Range;

use windows::core;

use crate::{
    command::{self, Command},
    request::Keys,
};

pub struct ValidateData(pub(crate) Keys);

impl ValidateData {
    pub fn pass(&self, range: Range<u64>) -> core::Result<()> {
        command::Validate { range }.execute(self.0, None)
    }
}

pub struct Dehydrate(pub(crate) Keys);

impl Dehydrate {
    pub fn pass(&self) -> core::Result<()> {
        command::Dehydrate {}.execute(self.0, None)
    }
}

pub struct Delete(pub(crate) Keys);

impl Delete {
    pub fn pass(&self) -> core::Result<()> {
        command::Delete.execute(self.0, None)
    }
}

pub struct Rename(pub(crate) Keys);

impl Rename {
    pub fn pass(&self) -> core::Result<()> {
        command::Rename.execute(self.0, None)
    }
}
