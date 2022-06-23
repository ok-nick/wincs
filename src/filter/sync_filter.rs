use crate::{
    error::CloudErrorKind,
    filter::{info, ticket},
    request::Request,
};

/// Core functions for implementing a Sync Engine.
///
/// `Send` and `Sync` are required as the callback could be invoked from an arbitrary thread, [read
/// here](https://docs.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_callback_type#remarks).
pub trait SyncFilter: Send + Sync {
    /// A placeholder hydration has been requested. This means that the placeholder should be
    /// populated with its corresponding data on the remote.
    fn fetch_data(&self, _request: Request, ticket: ticket::FetchData, _info: info::FetchData) {
        #[allow(unused_must_use)]
        {
            ticket.fail(CloudErrorKind::NotSupported);
        }
    }

    /// A placeholder hydration request has been cancelled.
    fn cancel_fetch_data(&self, _request: Request, _info: info::CancelFetchData) {}

    /// Followed by a successful call to [SyncFilter::fetch_data][crate::SyncFilter::fetch_data], this callback should verify the integrity of
    /// the data persisted in the placeholder.
    ///
    /// **You** are responsible for validating the data in the placeholder. To approve or
    /// disapprove the request, use the ticket provided.
    ///
    /// Note that this callback is only called if [HydrationPolicy::require_validation][crate::HydrationPolicy::require_validation] is specified.
    fn validate_data(
        &self,
        _request: Request,
        ticket: ticket::ValidateData,
        _info: info::ValidateData,
    ) {
        #[allow(unused_must_use)]
        {
            ticket.fail(CloudErrorKind::NotSupported);
        }
    }

    /// A directory population has been requested. The behavior of this callback is dependent on
    /// the [PopulationType][crate::PopulationType] variant specified during registration.
    fn fetch_placeholders(
        &self,
        _request: Request,
        ticket: ticket::FetchPlaceholders,
        _info: info::FetchPlaceholders,
    ) {
        #[allow(unused_must_use)]
        {
            ticket.fail(CloudErrorKind::NotSupported);
        }
    }

    /// A directory population request has been cancelled.
    fn cancel_fetch_placeholders(&self, _request: Request, _info: info::CancelFetchPlaceholders) {}

    /// A placeholder file handle has been opened for read, write, and/or delete
    /// access.
    fn opened(&self, _request: Request, _info: info::Opened) {}

    /// A placeholder file handle that has been previously opened with read, write,
    /// and/or delete access has been closed.
    fn closed(&self, _request: Request, _info: info::Closed) {}

    /// A placeholder dehydration has been requested. This means that all of the data persisted in
    /// the file will be __completely__ discarded.
    ///
    /// The operating system will handle dehydrating placeholder files automatically. However, it
    /// is up to **you** to approve this. Use the ticket to approve or disapprove the request.
    fn dehydrate(&self, _request: Request, ticket: ticket::Dehydrate, _info: info::Dehydrate) {
        #[allow(unused_must_use)]
        {
            ticket.fail(CloudErrorKind::NotSupported);
        }
    }

    /// A placeholder dehydration request has been cancelled.
    fn dehydrated(&self, _request: Request, _info: info::Dehydrated) {}

    /// A placeholder file is about to be deleted.
    ///
    /// The operating system will handle deleting placeholder files automatically. However, it is
    /// up to **you** to approve this. Use the ticket to approve or disapprove the request.
    fn delete(&self, _request: Request, ticket: ticket::Delete, _info: info::Delete) {
        #[allow(unused_must_use)]
        {
            ticket.fail(CloudErrorKind::NotSupported);
        }
    }

    /// A placeholder file has been deleted.
    fn deleted(&self, _request: Request, _info: info::Deleted) {}

    /// A placeholder file is about to be renamed or moved.
    ///
    /// The operating system will handle moving and renaming placeholder files automatically.
    /// However, it is up to **you** to approve this. Use the ticket to approve or disapprove the
    /// request.
    ///
    /// When the operation is completed, the [SyncFilter::renamed][crate::SyncFilter::renamed] callback will be called.
    fn rename(&self, _request: Request, ticket: ticket::Rename, _info: info::Rename) {
        #[allow(unused_must_use)]
        {
            ticket.fail(CloudErrorKind::NotSupported);
        }
    }

    /// A placeholder file has been renamed or moved.
    fn renamed(&self, _request: Request, _info: info::Renamed) {}
}
