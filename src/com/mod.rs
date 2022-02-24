#![allow(clippy::forget_copy)]

pub mod thumbnails;

// move this
pub enum SourceStatus {
    FileNotFound,
    NoSyncRoot,
}

// use windows::{core::RuntimeType, Foundation::Collections::IIterator};

// use windows as Windows;

//

// TODO: windows-rs updated and no longer uses the implement macro to define COM interfaces, instead it uses traits.
// This is temporarily unimplemented as the storage provider status source interface isn't documented enough to use

//

// // TODO: windows-rs doesn't have support for custom generics yet so the functionality here is a bit limited
// // However, I could technically use boxed trait objects
// #[implement(Windows::Foundation::Collections::IIterable<T>)]
// pub struct VecIterable<T: RuntimeType + 'static>(pub(crate) Vec<T>);

// #[allow(non_snake_case)]
// impl<T: RuntimeType + 'static> VecIterable<T> {
//     pub fn First(&self) -> windows::core::Result<IIterator<T>> {
//         Ok(VecIterator {
//             vec: self.0.clone(),
//             index: AtomicUsize::new(0),
//         }
//         .into())
//     }
// }

// #[implement(Windows::Foundation::Collections::IIterator<T>)]
// pub struct VecIterator<T: RuntimeType + 'static> {
//     vec: Vec<T>,
//     index: AtomicUsize,
// }

// #[allow(non_snake_case)]
// impl<T: RuntimeType + 'static> VecIterator<T> {
//     pub fn Current(&self) -> windows::core::Result<T> {
//         // the index is guaranteed to be in range
//         // TODO: drain the vector and take owned values
//         Ok(self
//             .vec
//             .get(self.index.load(Ordering::SeqCst))
//             .unwrap()
//             .clone())
//     }

//     pub fn HasCurrent(&self) -> windows::core::Result<bool> {
//         Ok(self.index.load(Ordering::SeqCst) <= self.vec.len())
//     }

//     pub fn MoveNext(&self) -> windows::core::Result<bool> {
//         self.index.fetch_add(1, Ordering::SeqCst);
//         self.HasCurrent()
//     }

//     pub fn GetMany(&self, items: &mut [T]) -> windows::core::Result<u32> {
//         let count = cmp::min(
//             items.len(),
//             self.vec.len() - self.index.load(Ordering::SeqCst),
//         );
//         if count > 0 {
//             self.index.fetch_add(count, Ordering::SeqCst);
//             // TODO: I need to swap them out instead of copying
//             unsafe { ptr::copy_nonoverlapping(self.vec.as_ptr(), items.as_mut_ptr(), count) }
//         }
//         Ok(count as u32)
//     }
// }
