use windows::{
    core,
    Win32::Storage::CloudFilters::{CfDisconnectSyncRoot, CF_CONNECTION_KEY},
};

use crate::{filter::Callbacks, request::RawConnectionKey};

#[derive(Debug)]
pub struct Session<T> {
    connection_key: RawConnectionKey,
    _callbacks: Callbacks,
    filter: T,
}

// this struct could house a bunch more windows api functions, although they all seem to do nothing
// according to the threads on microsoft q&a
impl<T> Session<T> {
    pub(crate) fn new(connection_key: RawConnectionKey, callbacks: Callbacks, filter: T) -> Self {
        Self {
            connection_key,
            _callbacks: callbacks,
            filter,
        }
    }

    pub fn connection_key(&self) -> RawConnectionKey {
        self.connection_key
    }

    pub fn filter(&self) -> &T {
        &self.filter
    }

    pub fn disconnect(self) -> core::Result<()> {
        self.disconnect_ref()
    }

    #[inline]
    fn disconnect_ref(&self) -> core::Result<()> {
        unsafe { CfDisconnectSyncRoot(&CF_CONNECTION_KEY(self.connection_key)) }
    }
}

impl<T> Drop for Session<T> {
    fn drop(&mut self) {
        #[allow(unused_must_use)]
        {
            self.disconnect_ref();
        }
    }
}
