/// Information for callbacks in the [SyncFilter][crate::SyncFilter] trait.
pub mod info;
/// Tickets for callbacks in the [SyncFilter][crate::SyncFilter] trait.
pub mod ticket;

pub(crate) use proxy::{callbacks, Callbacks};
pub use sync_filter::SyncFilter;

mod proxy;
mod sync_filter;
