use std::sync::Arc;

use windows::{
    core,
    Win32::Storage::CloudFilters::{CfDisconnectSyncRoot, CF_CONNECTION_KEY},
};

#[derive(Debug, Clone)]
pub struct Provider<T> {
    connection_key: isize,
    filter: Arc<T>,
}

impl<T> Provider<T> {
    pub(crate) fn new(connection_key: isize, filter: Arc<T>) -> Self {
        Self {
            connection_key,
            filter,
        }
    }

    pub fn connection_key(&self) -> isize {
        self.connection_key
    }

    pub fn filter(&self) -> Arc<T> {
        self.filter.clone()
    }

    pub fn disconnect(&self) -> core::Result<()> {
        unsafe { CfDisconnectSyncRoot(&CF_CONNECTION_KEY(self.connection_key)) }
    }
}

impl<T> Drop for Provider<T> {
    fn drop(&mut self) {
        // it is suggested to manually disconnect to handle errors although it's
        // always best to prevent possibilities of a memory leak
        #[allow(unused_must_use)]
        {
            self.disconnect();
        }
    }
}
