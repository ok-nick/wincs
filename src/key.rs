use std::{borrow::Borrow, fs::File, ops::Deref, os::windows::prelude::AsRawHandle};

use windows::{
    core,
    Win32::{
        Foundation::HANDLE,
        Storage::CloudFilters::{CfDisconnectSyncRoot, CfReleaseTransferKey, CF_CONNECTION_KEY},
    },
};

#[derive(Debug)]
pub struct OwnedConnectionKey<T> {
    key: isize,
    // this is the array of callbacks and the filter passed to a connection
    // when the connection is dropped, so should the context
    context: T,
}

impl<T> OwnedConnectionKey<T> {
    pub(crate) fn new(key: isize, context: T) -> Self {
        Self { key, context }
    }

    pub fn context(&self) -> &T {
        &self.context
    }

    pub fn close(self) -> core::Result<()> {
        self._close()
    }

    pub(crate) fn _close(&self) -> core::Result<()> {
        unsafe { CfDisconnectSyncRoot(&CF_CONNECTION_KEY(self.key)) }
    }
}

impl<T> AsRef<BorrowedConnectionKey> for OwnedConnectionKey<T> {
    fn as_ref(&self) -> &BorrowedConnectionKey {
        self
    }
}

impl<T> Deref for OwnedConnectionKey<T> {
    type Target = BorrowedConnectionKey;

    fn deref(&self) -> &Self::Target {
        BorrowedConnectionKey::new(&self.key)
    }
}

impl<T> Borrow<BorrowedConnectionKey> for OwnedConnectionKey<T> {
    fn borrow(&self) -> &BorrowedConnectionKey {
        self.deref()
    }
}

impl<T> Drop for OwnedConnectionKey<T> {
    fn drop(&mut self) {
        #[allow(unused_must_use)]
        {
            self._close();
        }
    }
}

#[derive(Debug)]
pub struct BorrowedConnectionKey(isize);

impl BorrowedConnectionKey {
    pub(crate) fn new(key: &isize) -> &Self {
        unsafe { &*(key as *const _ as *const Self) }
    }

    pub fn key(&self) -> &isize {
        &self.0
    }
}

impl AsRef<BorrowedConnectionKey> for BorrowedConnectionKey {
    fn as_ref(&self) -> &BorrowedConnectionKey {
        self
    }
}

#[derive(Debug)]
pub struct OwnedTransferKey {
    key: i64,
    // this could instead be an OwnedHandle, whenever that is stabilized
    file: File,
}

impl OwnedTransferKey {
    pub(crate) fn new(key: i64, file: File) -> Self {
        Self { key, file }
    }

    pub fn file(&self) -> &File {
        &self.file
    }
}

impl AsRef<BorrowedTransferKey> for OwnedTransferKey {
    fn as_ref(&self) -> &BorrowedTransferKey {
        self
    }
}

impl Deref for OwnedTransferKey {
    type Target = BorrowedTransferKey;

    fn deref(&self) -> &Self::Target {
        BorrowedTransferKey::new(&self.key)
    }
}

impl Borrow<BorrowedTransferKey> for OwnedTransferKey {
    fn borrow(&self) -> &BorrowedTransferKey {
        self.deref()
    }
}

impl Drop for OwnedTransferKey {
    fn drop(&mut self) {
        unsafe {
            CfReleaseTransferKey(
                HANDLE(self.file.as_raw_handle() as isize),
                self.key as *mut _,
            )
        }
    }
}

#[derive(Debug)]
pub struct BorrowedTransferKey(i64);

impl BorrowedTransferKey {
    pub(crate) fn new(key: &i64) -> &Self {
        unsafe { &*(key as *const _ as *const Self) }
    }

    pub fn key(&self) -> &i64 {
        &self.0
    }
}

impl AsRef<BorrowedTransferKey> for BorrowedTransferKey {
    fn as_ref(&self) -> &BorrowedTransferKey {
        self
    }
}
