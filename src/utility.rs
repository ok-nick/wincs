use windows::core::{self, HSTRING};

use crate::sealed;

pub use nt_time::FileTime;

// TODO: add something to convert an Option<T> to a *const T and *mut T

pub(crate) trait ToHString
where
    Self: AsRef<[u16]>,
{
    /// Converts a 16-bit buffer to a Windows reference-counted [HSTRING][windows::core::HSTRING].
    fn to_hstring(&self) -> HSTRING {
        HSTRING::from_wide(self.as_ref())
    }
}

impl<T: AsRef<[u16]>> ToHString for T {}

pub trait ReadAt: sealed::Sealed {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> core::Result<u64>;
}

pub trait WriteAt: sealed::Sealed {
    fn write_at(&self, buf: &[u8], offset: u64) -> core::Result<()>;
}
