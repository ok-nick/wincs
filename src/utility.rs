use windows::core::HSTRING;

// TODO: add something to convert an Option<T> to a *const T and *mut T

pub trait ToHString
where
    Self: AsRef<[u16]>,
{
    /// Converts a 16-bit buffer to a Windows reference-counted [HSTRING][windows::core::HSTRING].
    fn to_hstring(&self) -> HSTRING {
        HSTRING::from_wide(self.as_ref())
    }
}

impl<T: AsRef<[u16]>> ToHString for T {}
