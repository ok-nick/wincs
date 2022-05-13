// TODO: this module will be for handling usn's and potentially change journals.
// Usn's should ALWAYS be required and this module should make it easy

/// An Updated Sequence Number (USN) is as an identifier representing the version of a file. Each operation
/// performed will increment the USN of a file. This identifier is typically used in conjunction
/// with the NTFS Change Journal.
pub type Usn = u64;
