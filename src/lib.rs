#![doc = "../README.md"]

pub mod error;
/// Contains traits extending common structs from the [std][std].
pub mod ext;
pub mod filter;
pub mod metadata;
pub mod placeholder;
pub mod placeholder_file;
pub mod request;
pub mod root;
pub mod usn;
pub mod utility;

/// Contains low-level structs for directly executing Cloud Filter operations.
///
/// The [command][crate::command] API is exposed through various higher-level structs, like
/// [Request][crate::request::Request] and [Placeholder][crate::placeholder::Placeholder].
mod command;

mod sealed {
    pub trait Sealed {}
}
