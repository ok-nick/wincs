use windows::{
    core,
    Win32::Storage::CloudFilters::{CfDisconnectSyncRoot, CF_CONNECTION_KEY},
};

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
    _callbacks: Callbacks,
    filter: T,
}

// this struct could house many more windows api functions, although they all seem to do nothing
// according to the threads on microsoft q&a
impl<T> Connection<T> {
    pub(crate) fn new(connection_key: RawConnectionKey, callbacks: Callbacks, filter: T) -> Self {
        Self {
            connection_key,
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

    /// Disconnects the sync root, read [Connection][crate::Connection] for more information.
    pub fn disconnect(self) -> core::Result<()> {
        self.disconnect_ref()
    }

    #[inline]
    fn disconnect_ref(&self) -> core::Result<()> {
        unsafe { CfDisconnectSyncRoot(CF_CONNECTION_KEY(self.connection_key)) }
    }
}

impl<T> Drop for Connection<T> {
    fn drop(&mut self) {
        #[allow(unused_must_use)]
        {
            self.disconnect_ref();
        }
    }
}
