use std::{ops::Range, ptr};

use windows::{
    core,
    Win32::{
        Foundation,
        Storage::CloudFilters::{
            self, CF_OPERATION_PARAMETERS_0, CF_OPERATION_PARAMETERS_0_0,
            CF_OPERATION_PARAMETERS_0_1, CF_OPERATION_PARAMETERS_0_2, CF_OPERATION_PARAMETERS_0_3,
            CF_OPERATION_PARAMETERS_0_4, CF_OPERATION_PARAMETERS_0_5, CF_OPERATION_PARAMETERS_0_6,
            CF_OPERATION_PARAMETERS_0_7, CF_OPERATION_TYPE,
        },
    },
};

use crate::{
    command::{execute, Command, Fallible},
    error::CloudErrorKind,
    placeholder_file::{Metadata, PlaceholderFile},
    request::Keys,
};

#[derive(Debug)]
pub struct Read<'a> {
    pub buffer: &'a mut [u8],
    pub position: u64,
}

impl Command for Read<'_> {
    const OPERATION: CF_OPERATION_TYPE = CloudFilters::CF_OPERATION_TYPE_RETRIEVE_DATA;

    type Result = u64;
    type Field = CF_OPERATION_PARAMETERS_0_5;

    unsafe fn result(info: CF_OPERATION_PARAMETERS_0) -> Self::Result {
        info.RetrieveData.ReturnedLength as u64
    }

    fn build(&self) -> CF_OPERATION_PARAMETERS_0 {
        CF_OPERATION_PARAMETERS_0 {
            RetrieveData: CF_OPERATION_PARAMETERS_0_5 {
                Flags: CloudFilters::CF_OPERATION_RETRIEVE_DATA_FLAG_NONE,
                Buffer: self.buffer.as_ptr() as *mut _,
                Offset: self.position as i64,
                Length: self.buffer.len() as i64,
                ReturnedLength: 0,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Write<'a> {
    pub buffer: &'a [u8],
    pub position: u64,
}

impl Command for Write<'_> {
    const OPERATION: CF_OPERATION_TYPE = CloudFilters::CF_OPERATION_TYPE_TRANSFER_DATA;

    type Result = ();
    type Field = CF_OPERATION_PARAMETERS_0_6;

    unsafe fn result(_info: CF_OPERATION_PARAMETERS_0) -> Self::Result {}

    fn build(&self) -> CF_OPERATION_PARAMETERS_0 {
        CF_OPERATION_PARAMETERS_0 {
            TransferData: CF_OPERATION_PARAMETERS_0_6 {
                Flags: CloudFilters::CF_OPERATION_TRANSFER_DATA_FLAG_NONE,
                CompletionStatus: Foundation::STATUS_SUCCESS,
                Buffer: self.buffer.as_ptr() as *mut _,
                Offset: self.position as i64,
                Length: self.buffer.len() as i64,
            },
        }
    }
}

impl Fallible for Write<'_> {
    fn fail(keys: Keys, error_kind: CloudErrorKind) -> core::Result<Self::Result> {
        execute::<Self>(
            CF_OPERATION_PARAMETERS_0 {
                TransferData: CF_OPERATION_PARAMETERS_0_6 {
                    Flags: CloudFilters::CF_OPERATION_TRANSFER_DATA_FLAG_NONE,
                    CompletionStatus: error_kind.into(),
                    // TODO: SAME HERE AS BELOW?
                    Buffer: [0; 1].as_mut_ptr() as *mut _,
                    Offset: 0,
                    // TODO: DOES THIS HAVE TO BE DEFINED?
                    Length: 0,
                },
            },
            keys,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Update<'a> {
    pub mark_in_sync: bool,
    pub metadata: Option<Metadata>,
    pub blob: Option<&'a [u8]>,
}

impl Command for Update<'_> {
    const OPERATION: CF_OPERATION_TYPE = CloudFilters::CF_OPERATION_TYPE_RESTART_HYDRATION;

    type Result = ();
    type Field = CF_OPERATION_PARAMETERS_0_4;

    unsafe fn result(_info: CF_OPERATION_PARAMETERS_0) -> Self::Result {}

    fn build(&self) -> CF_OPERATION_PARAMETERS_0 {
        CF_OPERATION_PARAMETERS_0 {
            RestartHydration: CF_OPERATION_PARAMETERS_0_4 {
                Flags: if self.mark_in_sync {
                    CloudFilters::CF_OPERATION_RESTART_HYDRATION_FLAG_MARK_IN_SYNC
                } else {
                    CloudFilters::CF_OPERATION_RESTART_HYDRATION_FLAG_NONE
                },
                FsMetadata: self.metadata.map_or(ptr::null_mut(), |mut metadata| {
                    &mut metadata as *mut _ as *mut _
                }),
                FileIdentity: self
                    .blob
                    .map_or(ptr::null_mut(), |blob| blob.as_ptr() as *mut _),
                FileIdentityLength: self.blob.map_or(0, |blob| blob.len() as u32),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreatePlaceholders<'a> {
    pub placeholders: &'a [PlaceholderFile],
    pub total: u64,
}

impl Command for CreatePlaceholders<'_> {
    const OPERATION: CF_OPERATION_TYPE = CloudFilters::CF_OPERATION_TYPE_TRANSFER_PLACEHOLDERS;

    type Result = u32;
    type Field = CF_OPERATION_PARAMETERS_0_7;

    unsafe fn result(info: CF_OPERATION_PARAMETERS_0) -> Self::Result {
        info.TransferPlaceholders.EntriesProcessed
    }

    fn build(&self) -> CF_OPERATION_PARAMETERS_0 {
        CF_OPERATION_PARAMETERS_0 {
            TransferPlaceholders: CF_OPERATION_PARAMETERS_0_7 {
                Flags: CloudFilters::CF_OPERATION_TRANSFER_PLACEHOLDERS_FLAG_NONE,
                CompletionStatus: Foundation::STATUS_SUCCESS,
                PlaceholderTotalCount: self.total as i64,
                PlaceholderArray: self.placeholders.as_ptr() as *mut _,
                PlaceholderCount: self.placeholders.len() as u32,
                EntriesProcessed: 0,
            },
        }
    }
}

impl<'a> Fallible for CreatePlaceholders<'a> {
    fn fail(keys: Keys, error_kind: CloudErrorKind) -> core::Result<Self::Result> {
        execute::<Self>(
            CF_OPERATION_PARAMETERS_0 {
                TransferPlaceholders: CF_OPERATION_PARAMETERS_0_7 {
                    Flags: CloudFilters::CF_OPERATION_TRANSFER_PLACEHOLDERS_FLAG_NONE,
                    CompletionStatus: error_kind.into(),
                    PlaceholderTotalCount: 0,
                    // TODO: DOES THIS HAVE TO BE A VALID ARRAY?
                    PlaceholderArray: ptr::null_mut(),
                    PlaceholderCount: 0,
                    EntriesProcessed: 0,
                },
            },
            keys,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Validate {
    pub range: Range<u64>,
}

impl Command for Validate {
    const OPERATION: CF_OPERATION_TYPE = CloudFilters::CF_OPERATION_TYPE_ACK_DATA;

    type Result = ();
    type Field = CF_OPERATION_PARAMETERS_0_0;

    unsafe fn result(_info: CF_OPERATION_PARAMETERS_0) -> Self::Result {}

    fn build(&self) -> CF_OPERATION_PARAMETERS_0 {
        CF_OPERATION_PARAMETERS_0 {
            AckData: CF_OPERATION_PARAMETERS_0_0 {
                Flags: CloudFilters::CF_OPERATION_ACK_DATA_FLAG_NONE,
                CompletionStatus: Foundation::STATUS_SUCCESS,
                Offset: self.range.start as i64,
                Length: self.range.end as i64,
            },
        }
    }
}

impl Fallible for Validate {
    fn fail(keys: Keys, error_kind: CloudErrorKind) -> core::Result<Self::Result> {
        execute::<Self>(
            CF_OPERATION_PARAMETERS_0 {
                AckData: CF_OPERATION_PARAMETERS_0_0 {
                    Flags: CloudFilters::CF_OPERATION_ACK_DATA_FLAG_NONE,
                    CompletionStatus: error_kind.into(),
                    Offset: 0,
                    Length: 0,
                },
            },
            keys,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Dehydrate<'a> {
    pub blob: Option<&'a [u8]>,
}

impl Command for Dehydrate<'_> {
    const OPERATION: CF_OPERATION_TYPE = CloudFilters::CF_OPERATION_TYPE_ACK_DEHYDRATE;

    type Result = ();
    type Field = CF_OPERATION_PARAMETERS_0_1;

    unsafe fn result(_info: CF_OPERATION_PARAMETERS_0) -> Self::Result {}

    fn build(&self) -> CF_OPERATION_PARAMETERS_0 {
        CF_OPERATION_PARAMETERS_0 {
            AckDehydrate: CF_OPERATION_PARAMETERS_0_1 {
                Flags: CloudFilters::CF_OPERATION_ACK_DEHYDRATE_FLAG_NONE,
                CompletionStatus: Foundation::STATUS_SUCCESS,
                FileIdentity: self
                    .blob
                    .map_or(ptr::null(), |blob| blob.as_ptr() as *const _),
                FileIdentityLength: self.blob.map_or(0, |blob| blob.len() as u32),
            },
        }
    }
}

impl Fallible for Dehydrate<'_> {
    fn fail(keys: Keys, error_kind: CloudErrorKind) -> core::Result<Self::Result> {
        execute::<Self>(
            CF_OPERATION_PARAMETERS_0 {
                AckDehydrate: CF_OPERATION_PARAMETERS_0_1 {
                    Flags: CloudFilters::CF_OPERATION_ACK_DEHYDRATE_FLAG_NONE,
                    CompletionStatus: error_kind.into(),
                    FileIdentity: ptr::null(),
                    FileIdentityLength: 0,
                },
            },
            keys,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Delete;

impl Command for Delete {
    const OPERATION: CF_OPERATION_TYPE = CloudFilters::CF_OPERATION_TYPE_ACK_DELETE;

    type Result = ();
    type Field = CF_OPERATION_PARAMETERS_0_2;

    unsafe fn result(_info: CF_OPERATION_PARAMETERS_0) -> Self::Result {}

    fn build(&self) -> CF_OPERATION_PARAMETERS_0 {
        CF_OPERATION_PARAMETERS_0 {
            AckDelete: CF_OPERATION_PARAMETERS_0_2 {
                Flags: CloudFilters::CF_OPERATION_ACK_DELETE_FLAG_NONE,
                CompletionStatus: Foundation::STATUS_SUCCESS,
            },
        }
    }
}

impl Fallible for Delete {
    fn fail(keys: Keys, error_kind: CloudErrorKind) -> core::Result<Self::Result> {
        execute::<Self>(
            CF_OPERATION_PARAMETERS_0 {
                AckDelete: CF_OPERATION_PARAMETERS_0_2 {
                    Flags: CloudFilters::CF_OPERATION_ACK_DELETE_FLAG_NONE,
                    CompletionStatus: error_kind.into(),
                },
            },
            keys,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rename;

impl Command for Rename {
    const OPERATION: CF_OPERATION_TYPE = CloudFilters::CF_OPERATION_TYPE_ACK_RENAME;

    type Result = ();
    type Field = CF_OPERATION_PARAMETERS_0_3;

    unsafe fn result(_info: CF_OPERATION_PARAMETERS_0) -> Self::Result {}

    fn build(&self) -> CF_OPERATION_PARAMETERS_0 {
        CF_OPERATION_PARAMETERS_0 {
            AckRename: CF_OPERATION_PARAMETERS_0_3 {
                Flags: CloudFilters::CF_OPERATION_ACK_RENAME_FLAG_NONE,
                CompletionStatus: Foundation::STATUS_SUCCESS,
            },
        }
    }
}

impl Fallible for Rename {
    fn fail(keys: Keys, error_kind: CloudErrorKind) -> core::Result<Self::Result> {
        execute::<Self>(
            CF_OPERATION_PARAMETERS_0 {
                AckRename: CF_OPERATION_PARAMETERS_0_3 {
                    Flags: CloudFilters::CF_OPERATION_ACK_RENAME_FLAG_NONE,
                    CompletionStatus: error_kind.into(),
                },
            },
            keys,
        )
    }
}
