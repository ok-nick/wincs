use std::ops::{BitAndAssign, BitOrAssign, Not};

use windows::core::HSTRING;

pub fn hstring_from_widestring<T: AsRef<[u16]>>(bytes: T) -> HSTRING {
    HSTRING::from_wide(bytes.as_ref())
}

pub fn set_flag<T>(flags: &mut T, flag: T, yes: bool)
where
    T: BitOrAssign + BitAndAssign + Not<Output = T>,
{
    if yes {
        *flags |= flag;
    } else {
        *flags &= !flag;
    }
}
