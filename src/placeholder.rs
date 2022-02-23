use std::{
    io::{self, Seek, SeekFrom},
    mem::ManuallyDrop,
    os::windows::prelude::AsRawHandle,
    path::PathBuf,
    ptr,
};

use widestring::U16CString;
use windows::{
    core::{self, GUID},
    Win32::{
        Storage::{
            CloudFilters::{CfReportProviderProgress, CF_CONNECTION_KEY},
            EnhancedStorage,
        },
        System::{
            Com::StructuredStorage::{
                PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
            },
            Ole::VT_UI4,
        },
        UI::Shell::{
            self, IShellItem2,
            PropertiesSystem::{
                self, IPropertyStore, InitPropVariantFromUInt64Vector, PROPERTYKEY,
            },
            SHChangeNotify, SHCreateItemFromParsingName,
        },
    },
};

use crate::{
    command::{commands::Read, Command, Update, Write},
    placeholder_file::Metadata,
    request::Keys,
};

// secret PKEY
const STORAGE_PROVIDER_TRANSFER_PROGRESS: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_values(
        0xE77E90DF,
        0x6271,
        0x4F5B,
        [0x83, 0x4F, 0x2D, 0xD1, 0xF2, 0x45, 0xDD, 0xA4],
    ),
    pid: 4,
};

#[derive(Debug, Clone)]
pub struct Placeholder {
    keys: Keys,
    path: PathBuf,
    // TODO: File size should update when writing past the end of a file
    file_size: u64,
    position: u64,
}

impl Placeholder {
    pub(crate) fn new(keys: Keys, path: PathBuf, file_size: u64) -> Self {
        Self {
            keys,
            path,
            file_size,
            position: 0,
        }
    }

    // TODO: Is a connection key necessary?
    // CfGetTransferKey
    // according to this post it looks optional
    // https://stackoverflow.com/questions/66988096/windows-10-file-cloud-sync-provider-api-transferdata-problem
    pub fn from_file<T: AsRawHandle>(file: T) -> Self {
        todo!()
    }

    // TODO: getters

    // TODO: add a single function w/ an options/builder struct for all three funcs below
    pub fn mark_in_sync(&self) -> core::Result<()> {
        Update {
            mark_in_sync: true,
            metadata: None,
            blob: None,
        }
        .execute(self.keys, None)
    }

    pub fn set_metadata(&self, metadata: Metadata) -> core::Result<()> {
        Update {
            mark_in_sync: false,
            metadata: Some(metadata),
            blob: None,
        }
        .execute(self.keys, None)
    }

    pub fn set_blob(mut self, blob: &[u8]) -> core::Result<()> {
        assert!(
            blob.len() <= 4096,
            "blob size must not exceed 4KB (4096 bytes) after serialization, got {} bytes",
            blob.len()
        );

        Update {
            mark_in_sync: false,
            metadata: None,
            blob: Some(blob),
        }
        .execute(self.keys, None)
    }

    pub fn set_progress(&self, total: u64, completed: u64) -> core::Result<()> {
        unsafe {
            CfReportProviderProgress(
                CF_CONNECTION_KEY(self.keys.connection_key),
                self.keys.transfer_key,
                total as i64,
                completed as i64,
            )?;

            let item: IShellItem2 = SHCreateItemFromParsingName(self.path.as_os_str(), None)?;
            let store: IPropertyStore = item.GetPropertyStore(
                PropertiesSystem::GPS_READWRITE | PropertiesSystem::GPS_VOLATILEPROPERTIESONLY,
            )?;

            let progress = InitPropVariantFromUInt64Vector(&mut [completed, total] as *mut _, 2)?;
            store.SetValue(
                &STORAGE_PROVIDER_TRANSFER_PROGRESS as *const _,
                &progress as *const _,
            )?;

            let status = InitPropVariantFromUInt32(if completed < total {
                PropertiesSystem::STS_TRANSFERRING.0
            } else {
                PropertiesSystem::STS_NONE.0
            });
            store.SetValue(
                &EnhancedStorage::PKEY_SyncTransferStatus as *const _,
                &status as *const _,
            )?;

            store.Commit()?;

            SHChangeNotify(
                Shell::SHCNE_UPDATEITEM,
                Shell::SHCNF_PATHW,
                U16CString::from_os_str_unchecked(self.path.as_os_str()).as_ptr() as *const _,
                ptr::null_mut(),
            );

            Ok(())
        }
    }
}

// TODO: does this have the same 4kb requirement as writing?
impl io::Read for Placeholder {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let result = Read {
            buffer,
            position: self.position,
        }
        .execute(self.keys, None);

        match result {
            Ok(bytes_read) => {
                self.position += bytes_read;
                Ok(bytes_read as usize)
            }
            Err(err) => Err(err.into()),
        }
    }
}

impl io::Write for Placeholder {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        assert!(
            buffer.len() % 4096 == 0 || self.position + buffer.len() as u64 >= self.file_size,
            "the length of the buffer must be 4kb aligned or ending on the logical file size"
        );

        let result = Write {
            buffer,
            position: self.position,
        }
        .execute(self.keys, None);

        match result {
            Ok(_) => {
                self.position += buffer.len() as u64;
                Ok(buffer.len())
            }
            Err(err) => Err(err.into()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// TODO: properly handle seeking
impl Seek for Placeholder {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.position = match position {
            SeekFrom::Start(offset) => offset,
            SeekFrom::Current(offset) => (self.position + offset as u64),
            SeekFrom::End(offset) => (self.file_size + offset as u64),
        };

        Ok(self.position)
    }
}

// Equivalent to https://docs.microsoft.com/en-us/windows/win32/api/propvarutil/nf-propvarutil-initpropvariantfromuint32
// windows-rs doesn't provide bindings to inlined functions
#[allow(non_snake_case)]
fn InitPropVariantFromUInt32(ulVal: u32) -> PROPVARIANT {
    PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                vt: VT_UI4.0 as u16,
                Anonymous: PROPVARIANT_0_0_0 { ulVal },
                ..Default::default()
            }),
        },
    }
}
