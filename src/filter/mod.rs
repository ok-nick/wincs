/// Information for callbacks in the [SyncFilter][crate::SyncFilter] trait.
pub mod info;
/// Tickets for callbacks in the [SyncFilter][crate::SyncFilter] trait.
pub mod ticket;

pub(crate) use async_filter::AsyncBridge;
pub use async_filter::Filter;
pub(crate) use proxy::{callbacks, Callbacks};
pub use sync_filter::SyncFilter;

mod async_filter;
mod proxy;
mod sync_filter;
