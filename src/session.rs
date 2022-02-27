use std::sync::Arc;

use windows::core;

use crate::key::{BorrowedConnectionKey, OwnedConnectionKey};

#[derive(Debug)]
pub struct Session<T>(OwnedConnectionKey<T>);

// this struct could house a bunch more windows api functions, although they all seem to do nothing
// according to the threads on microsoft q&a
impl<T> Session<T> {
    pub(crate) fn new(connection_key: OwnedConnectionKey<T>) -> Self {
        Self(connection_key)
    }

    pub fn connection_key(&self) -> &BorrowedConnectionKey {
        &self.0
    }

    pub fn disconnect(self) -> core::Result<()> {
        self.0._close()
    }
}

impl<T, U> Session<(U, Arc<T>)> {
    pub fn filter(&self) -> &Arc<T> {
        &self.0.context().1
    }
}

impl<T> Drop for Session<T> {
    fn drop(&mut self) {
        // it is suggested to manually disconnect to handle errors although it's
        // always best to prevent possibilities of a memory leak
        #[allow(unused_must_use)]
        {
            self.0._close();
        }
    }
}
