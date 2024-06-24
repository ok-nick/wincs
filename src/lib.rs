/// Contains low-level structs for directly executing Cloud Filter operations.
///
/// The [command][crate::command] API is exposed through various higher-level structs, like
/// [Request][crate::Request] and [Placeholder][crate::Placeholder]. Thus, it is not necessary to
/// create and call these structs manually unless you need more granular access.
pub mod command;
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

mod sealed {
    pub trait Sealed {}
}
