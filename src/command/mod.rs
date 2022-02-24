pub mod commands;

use std::{mem, ptr};

use memoffset::offset_of;
use windows::{
    core,
    Win32::Storage::CloudFilters::{
        CfExecute, CF_CONNECTION_KEY, CF_OPERATION_INFO, CF_OPERATION_PARAMETERS,
        CF_OPERATION_PARAMETERS_0, CF_OPERATION_TYPE,
    },
};

use crate::{error::CloudErrorKind, request::Keys};
pub use commands::*;

pub trait Fallible: Command {
    fn fail(keys: Keys, error_kind: CloudErrorKind) -> core::Result<Self::Result>;
}

pub trait Command: Sized {
    const OPERATION: CF_OPERATION_TYPE;
    type Result;
    type Field;

    /// # Safety
    /// Indexing a union
    unsafe fn result(info: CF_OPERATION_PARAMETERS_0) -> Self::Result;

    fn build(&self) -> CF_OPERATION_PARAMETERS_0;

    fn execute(&self, keys: Keys) -> core::Result<Self::Result> {
        execute::<Self>(self.build(), keys)
    }
}

pub fn execute<C: Command>(info: CF_OPERATION_PARAMETERS_0, keys: Keys) -> core::Result<C::Result> {
    unsafe {
        CfExecute(
            &CF_OPERATION_INFO {
                StructSize: mem::size_of::<CF_OPERATION_INFO>() as u32,
                Type: C::OPERATION,
                ConnectionKey: CF_CONNECTION_KEY(keys.connection_key),
                TransferKey: keys.transfer_key,
                CorrelationVector: ptr::null_mut(),
                SyncStatus: ptr::null(),
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
