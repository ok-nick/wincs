#![allow(clippy::missing_safety_doc)]

use std::sync::{Arc, Weak};

use windows::Win32::Storage::CloudFilters::{
    self, CF_CALLBACK_INFO, CF_CALLBACK_PARAMETERS, CF_CALLBACK_REGISTRATION,
};

use crate::{
    filter::{info, ticket, CallbackType, SyncFilter},
    request::Request,
};

macro_rules! callbacks {
    ($([$type:path, $name:ident]),*) => {
        [
            $(
                CF_CALLBACK_REGISTRATION {
                    Type: $type,
                    Callback: Some($name::<T>),
                },
            )*
            CF_CALLBACK_REGISTRATION {
                Type: CloudFilters::CF_CALLBACK_TYPE_NONE,
                Callback: None,
            },
        ]
    };
}

// TODO: const this
pub fn callbacks<T: SyncFilter + 'static>() -> [CF_CALLBACK_REGISTRATION; 14] {
    callbacks!(
        [CloudFilters::CF_CALLBACK_TYPE_FETCH_DATA, fetch_data],
        [CloudFilters::CF_CALLBACK_TYPE_VALIDATE_DATA, validate_data],
        [
            CloudFilters::CF_CALLBACK_TYPE_CANCEL_FETCH_DATA,
            cancel_fetch_data
        ],
        [
            CloudFilters::CF_CALLBACK_TYPE_FETCH_PLACEHOLDERS,
            fetch_placeholders
        ],
        [
            CloudFilters::CF_CALLBACK_TYPE_CANCEL_FETCH_PLACEHOLDERS,
            cancel_fetch_placeholders
        ],
        [
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_FILE_OPEN_COMPLETION,
            notify_file_open_completion
        ],
        [
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_FILE_CLOSE_COMPLETION,
            notify_file_close_completion
        ],
        [
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_DEHYDRATE,
            notify_dehydrate
        ],
        [
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_DEHYDRATE_COMPLETION,
            notify_dehydrate_completion
        ],
        [CloudFilters::CF_CALLBACK_TYPE_NOTIFY_DELETE, notify_delete],
        [
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_DELETE_COMPLETION,
            notify_rename
        ],
        [
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_RENAME,
            notify_rename_completion
        ],
        [
            CloudFilters::CF_CALLBACK_TYPE_NOTIFY_RENAME_COMPLETION,
            fetch_data
        ]
    )
}

pub unsafe extern "system" fn fetch_data<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    if let Some(filter) = filter_from_info::<T>(info) {
        filter.fetch_data(
            Request::new(*info, CallbackType::FetchData),
            info::FetchData((*params).Anonymous.FetchData),
        );
    }
}

pub unsafe extern "system" fn validate_data<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    let request = Request::new(*info, CallbackType::ValidateData);
    if let Some(filter) = filter_from_info::<T>(info) {
        let keys = request.keys();
        filter.validate_data(
            request,
            ticket::ValidateData(keys),
            info::ValidateData((*params).Anonymous.ValidateData),
        );
    }
}

pub unsafe extern "system" fn cancel_fetch_data<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    if let Some(filter) = filter_from_info::<T>(info) {
        filter.cancel_fetch_placeholders(
            Request::new(*info, CallbackType::FetchData),
            info::Cancel((*params).Anonymous.Cancel),
        );
    }
}

pub unsafe extern "system" fn fetch_placeholders<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    if let Some(filter) = filter_from_info::<T>(info) {
        filter.fetch_placeholders(
            Request::new(*info, CallbackType::FetchPlaceholders),
            info::FetchPlaceholders((*params).Anonymous.FetchPlaceholders),
        );
    }
}

pub unsafe extern "system" fn cancel_fetch_placeholders<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    if let Some(filter) = filter_from_info::<T>(info) {
        filter.cancel_fetch_placeholders(
            Request::new(*info, CallbackType::CancelFetchPlaceholders),
            info::Cancel((*params).Anonymous.Cancel),
        );
    }
}

pub unsafe extern "system" fn notify_file_open_completion<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    if let Some(filter) = filter_from_info::<T>(info) {
        filter.opened(
            Request::new(*info, CallbackType::Opened),
            info::Opened((*params).Anonymous.OpenCompletion),
        );
    }
}

pub unsafe extern "system" fn notify_file_close_completion<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    if let Some(filter) = filter_from_info::<T>(info) {
        filter.closed(
            Request::new(*info, CallbackType::Closed),
            info::Closed((*params).Anonymous.CloseCompletion),
        );
    }
}

pub unsafe extern "system" fn notify_dehydrate<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    let request = Request::new(*info, CallbackType::Rename);
    if let Some(filter) = filter_from_info::<T>(info) {
        let keys = request.keys();
        filter.dehydrate(
            request,
            ticket::Dehydrate(keys),
            info::Dehydrate((*params).Anonymous.Dehydrate),
        );
    }
}

pub unsafe extern "system" fn notify_dehydrate_completion<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    if let Some(filter) = filter_from_info::<T>(info) {
        filter.dehydrated(
            Request::new(*info, CallbackType::Dehydrate),
            info::Dehydrated((*params).Anonymous.DehydrateCompletion),
        );
    }
}

pub unsafe extern "system" fn notify_delete<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    let request = Request::new(*info, CallbackType::Delete);
    if let Some(filter) = filter_from_info::<T>(info) {
        let keys = request.keys();
        filter.delete(
            request,
            ticket::Delete(keys),
            info::Delete((*params).Anonymous.Delete),
        );
    }
}

pub unsafe extern "system" fn notify_delete_completion<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    if let Some(filter) = filter_from_info::<T>(info) {
        filter.deleted(
            Request::new(*info, CallbackType::Deleted),
            info::Deleted((*params).Anonymous.DeleteCompletion),
        );
    }
}

pub unsafe extern "system" fn notify_rename<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    let request = Request::new(*info, CallbackType::Rename);
    if let Some(filter) = filter_from_info::<T>(info) {
        let keys = request.keys();
        filter.rename(
            request,
            ticket::Rename(keys),
            info::Rename((*params).Anonymous.Rename),
        );
    }
}

pub unsafe extern "system" fn notify_rename_completion<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
    params: *const CF_CALLBACK_PARAMETERS,
) {
    if let Some(filter) = filter_from_info::<T>(info) {
        filter.renamed(
            Request::new(*info, CallbackType::Renamed),
            info::Renamed((*params).Anonymous.RenameCompletion),
        );
    }
}

unsafe fn filter_from_info<T: SyncFilter + 'static>(
    info: *const CF_CALLBACK_INFO,
) -> Option<Arc<T>> {
    // get the original weak arc
    let weak = Weak::from_raw((*info).CallbackContext as *mut T);
    // attempt to upgrade it to a strong arc
    match weak.upgrade() {
        // if the memory exists then the filter hasn't been disconnected
        Some(strong) => {
            // forget the original weak arc for next use
            let _ = Weak::into_raw(weak);
            // return the strong arc
            Some(strong)
        }
        // if the memory is freed then the filter is disconnected
        None => {
            // TODO: could a callback be called while the filter is being disconnected?
            // deallocate the weak pointer
            drop(weak);
            None
        }
    }
}
