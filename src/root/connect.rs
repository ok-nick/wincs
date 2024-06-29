use std::{
    sync::mpsc::Sender,
    thread::{self, JoinHandle},
    time::Duration,
};

use windows::Win32::Storage::CloudFilters::{CfDisconnectSyncRoot, CF_CONNECTION_KEY};

use crate::{filter::Callbacks, request::RawConnectionKey};

/// A handle to the current session for a given sync root.
///
/// By calling [Connection::disconnect][crate::Connection::disconnect], the session will terminate
/// and no more file operations will be able to be performed within the sync root. Note that this
/// does **NOT** mean the sync root will be unregistered. To do so, call
/// [SyncRootId::unregister][crate::SyncRootId::unregister].
///
/// [Connection::disconnect][crate::Connection::disconnect] is called implicitly when the struct is
/// dropped. To handle possible errors, be sure to call
/// [Connection::disconnect][crate::Connection::disconnect] explicitly.
#[derive(Debug)]
pub struct Connection<T> {
    connection_key: RawConnectionKey,

    cancel_token: Sender<()>,
    join_handle: JoinHandle<()>,

    _callbacks: Callbacks,
    filter: T,
}

// this struct could house many more windows api functions, although they all seem to do nothing
// according to the threads on microsoft q&a
impl<T> Connection<T> {
    pub(crate) fn new(
        connection_key: RawConnectionKey,
        cancel_token: Sender<()>,
        join_handle: JoinHandle<()>,
        callbacks: Callbacks,
        filter: T,
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

    /// A reference to the inner [SyncFilter][crate::SyncFilter] struct.
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
