use std::{
    sync::{mpsc::Sender, Arc},
    thread::{self, JoinHandle},
    time::Duration,
};

use windows::Win32::Storage::CloudFilters::{CfDisconnectSyncRoot, CF_CONNECTION_KEY};

use crate::{filter::Callbacks, request::RawConnectionKey};

/// A handle to the current session for a given sync root.
///
/// [Connection] will disconnect when dropped. Note that this
/// does **NOT** mean the sync root will be unregistered. To do so, call
/// [SyncRootId::unregister][crate::root::SyncRootId::unregister].
#[derive(Debug)]
pub struct Connection<F> {
    connection_key: RawConnectionKey,

    cancel_token: Sender<()>,
    join_handle: JoinHandle<()>,

    _callbacks: Callbacks,
    filter: Arc<F>,
}

// this struct could house many more windows api functions, although they all seem to do nothing
// according to the threads on microsoft q&a
impl<T> Connection<T> {
    pub(crate) fn new(
        connection_key: RawConnectionKey,
        cancel_token: Sender<()>,
        join_handle: JoinHandle<()>,
        callbacks: Callbacks,
        filter: Arc<T>,
    ) -> Self {
        Self {
            connection_key,
            cancel_token,
            join_handle,
            _callbacks: callbacks,
            filter,
        }
    }

    /// A raw connection key used to identify the connection.
    pub fn connection_key(&self) -> RawConnectionKey {
        self.connection_key
    }

    /// A reference to the inner [SyncFilter][crate::filter::SyncFilter] struct.
    pub fn filter(&self) -> &T {
        &self.filter
    }
}

impl<T> Drop for Connection<T> {
    fn drop(&mut self) {
        unsafe { CfDisconnectSyncRoot(CF_CONNECTION_KEY(self.connection_key)) }.unwrap();

        _ = self.cancel_token.send(());
        while !self.join_handle.is_finished() {
            thread::sleep(Duration::from_millis(150));
        }
    }
}
