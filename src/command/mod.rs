pub mod commands;

use std::{mem, ptr};

use memoffset::offset_of;
use widestring::U16CString;
use windows::{
    core,
    Win32::{
        Foundation,
        Storage::CloudFilters::{
            CfExecute, CF_CONNECTION_KEY, CF_OPERATION_INFO, CF_OPERATION_PARAMETERS,
            CF_OPERATION_PARAMETERS_0, CF_OPERATION_TYPE, CF_SYNC_STATUS,
        },
    },
};

use crate::{
    logger::{ErrorReason, Reason},
    request::Keys,
};
pub use commands::*;

pub trait Fallible: Command {
    fn fail(keys: Keys, reason: Option<Reason>) -> core::Result<Self::Result>;
}

pub trait Command: Sized {
    const OPERATION: CF_OPERATION_TYPE;
    type Result;
    type Field;

    /// # Safety
    /// Indexing a union
    unsafe fn result(info: CF_OPERATION_PARAMETERS_0) -> Self::Result;

    fn build(&self) -> CF_OPERATION_PARAMETERS_0;

    fn execute(&self, keys: Keys, reason: Option<Reason>) -> core::Result<Self::Result> {
        execute::<Self>(self.build(), keys, reason)
    }
}

pub fn execute<C: Command>(
    info: CF_OPERATION_PARAMETERS_0,
    keys: Keys,
    reason: Option<Reason>,
) -> core::Result<C::Result> {
    unsafe {
        let (status, len) = reason.map_or((ptr::null_mut(), 0), |reason| {
            let mut status = SyncStatus::from(reason);
            (
                &mut status as *mut _ as *mut CF_SYNC_STATUS,
                status.status.DescriptionLength,
            )
        });

        CfExecute(
            &CF_OPERATION_INFO {
                StructSize: (mem::size_of::<CF_OPERATION_INFO>() as u32) + len,
                Type: C::OPERATION,
                ConnectionKey: CF_CONNECTION_KEY(keys.connection_key),
                TransferKey: keys.transfer_key,
                CorrelationVector: ptr::null_mut(),
                SyncStatus: status,
                RequestKey: keys.request_key,
            } as *const _,
            &mut CF_OPERATION_PARAMETERS {
                ParamSize: (mem::size_of::<C::Field>()
                    + offset_of!(CF_OPERATION_PARAMETERS, Anonymous))
                    as u32,
                Anonymous: info,
            } as *mut _,
        )
        .and(Ok(C::result(info)))
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct SyncStatus {
    status: CF_SYNC_STATUS,
    description: *const u16,
}

// TODO: everything here could be done at compile time
impl From<Reason> for SyncStatus {
    fn from(reason: Reason) -> Self {
        let message = reason.message();
        Self {
            status: CF_SYNC_STATUS {
                StructSize: (mem::size_of::<Self>() + message.len()) as u32,
                Code: reason.code(),
                DescriptionOffset: offset_of!(Self, description) as u32,
                // add 1 for the null terminator
                DescriptionLength: (message.len() + 1) as u32,
                DeviceIdOffset: 0,
                DeviceIdLength: 0,
            },
            description: message.as_ptr(),
        }
    }
}
