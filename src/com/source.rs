// #![allow(clippy::forget_copy)]

// use windows::{
//     core::{implement, IInspectable},
//     Foundation::{Collections::IIterable, EventRegistrationToken, TypedEventHandler},
//     Storage::Provider::{
//         IStorageProviderStatusSource, StorageProviderError, StorageProviderStatus,
//     },
// };

// use windows as Windows;

// use crate::{com::VecIterable, logger::Logger, root::hstring_from_widestring};

// // TODO: there are no docs on how to register this
// // https://docs.microsoft.com/en-us/answers/questions/697756/istorageproviderhandlerfactory-how-to-register-for.html
// #[implement(Windows::Storage::Provider::IStorageProviderStatusSource)]
// pub struct Source(Box<dyn Logger>);

// #[allow(non_snake_case)]
// impl Source {
//     pub fn GetStatus(&self) -> windows::core::Result<StorageProviderStatus> {
//         StorageProviderStatus::CreateInstance2(
//             self.0.state().into(),
//             hstring_from_widestring(&self.0.message().to_ustring()),
//             IIterable::from(VecIterable(
//                 self.0
//                     .logs()
//                     .iter()
//                     .filter_map(|log| log.clone().try_into().ok())
//                     .collect::<Vec<StorageProviderError>>(),
//             )),
//         )
//     }

//     pub fn Changed(
//         &self,
//         handler: &Option<TypedEventHandler<IStorageProviderStatusSource, IInspectable>>,
//     ) -> windows::core::Result<EventRegistrationToken> {
//         todo!()
//     }

//     pub fn RemoveChanged(&self, token: &EventRegistrationToken) -> windows::core::Result<()> {
//         todo!()
//     }
// }

pub enum SourceStatus {
    FileNotFound,
    NotInSyncRoot,
}
