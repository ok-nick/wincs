use std::{mem, ptr};

use memoffset::offset_of;
use windows::{
    core,
    Win32::Storage::CloudFilters::{
        self, CfExecute, CF_CONNECTION_KEY, CF_OPERATION_INFO, CF_OPERATION_PARAMETERS,
        CF_OPERATION_PARAMETERS_0, CF_OPERATION_TYPE,
    },
};

use crate::{
    error::CloudErrorKind,
    request::{RawConnectionKey, RawTransferKey},
};

pub trait Fallible: Command {
    fn fail(
        connection_key: RawConnectionKey,
        transfer_key: RawTransferKey,
        error_kind: CloudErrorKind,
    ) -> core::Result<Self::Result>;
}

pub trait Command: Sized {
    const OPERATION: CF_OPERATION_TYPE;
    type Result;
    type Field;

    /// # Safety
    /// Indexing a union
    unsafe fn result(info: CF_OPERATION_PARAMETERS_0) -> Self::Result;

    fn build(&self) -> CF_OPERATION_PARAMETERS_0;

    fn execute(
        &self,
        connection_key: RawConnectionKey,
        transfer_key: RawTransferKey,
    ) -> core::Result<Self::Result> {
        execute::<Self>(self.build(), connection_key, transfer_key)
    }
}

pub fn execute<C: Command>(
    info: CF_OPERATION_PARAMETERS_0,
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
) -> core::Result<C::Result> {
    unsafe {
        CfExecute(
            &CF_OPERATION_INFO {
                StructSize: mem::size_of::<CF_OPERATION_INFO>() as u32,
                Type: C::OPERATION,
                ConnectionKey: CF_CONNECTION_KEY(connection_key),
                TransferKey: transfer_key,
                CorrelationVector: ptr::null_mut(),
                SyncStatus: ptr::null(),
                // https://docs.microsoft.com/en-us/answers/questions/749979/what-is-a-requestkey-cfapi.html
                RequestKey: CloudFilters::CF_REQUEST_KEY_DEFAULT as i64,
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
