use std::{
    io::{self, Seek, SeekFrom},
    mem::ManuallyDrop,
    path::{Path, PathBuf},
    ptr, ops::Range,
};

use widestring::U16CString;
use windows::{
    core::{self, GUID},
    Win32::{
        Storage::{
            CloudFilters::{self, CfReportProviderProgress, CF_CONNECTION_KEY},
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
    command::{Command, Read, Update, Validate, Write},
    placeholder_file::Metadata,
    request::{RawConnectionKey, RawTransferKey},
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

/// A struct to perform various operations on a placeholder file/directory.
#[derive(Debug, Clone)]
pub struct Placeholder {
    connection_key: RawConnectionKey,
    transfer_key: RawTransferKey,
    // TODO: take in a borrowed path
    path: PathBuf,
    // TODO: how does file size behave when writing past the last recorded file size?
    file_size: u64,
    position: u64,
}

impl Placeholder {
    pub(crate) fn new(
        connection_key: RawConnectionKey,
        transfer_key: RawTransferKey,
        path: PathBuf,
        file_size: u64,
    ) -> Self {
        Self {
            connection_key,
            transfer_key,
            path,
            file_size,
            position: 0,
        }
    }

    // TODO: this function is a bit of a pickle
    // if I let the user pass in the transfer key, then I somehow need to get the file path to support set_progress
    // if I allow getting a transfer key from a path, then I somehow need to let them specify read/write access (probably?)
    pub fn from_path<P: AsRef<Path>>(
        connection_key: RawConnectionKey,
        path: P,
    ) -> core::Result<Self> {
        // let file = File::open(path).unwrap();
        // let key = unsafe { CfGetTransferKey(HANDLE(file.as_raw_handle() as isize))?};
        // OwnedTransferKey::new(key, file);

        // Ok(Self {
        //     connection_key,

        // })
        todo!()
    }

    /// Validates the data range in the placeholder is valid.
    ///
    /// This method should be used in the `SyncFilter::validate_data` callback.
    ///
    /// This method is equivalent to `Validate::execute`.
    // TODO: accept a generic RangeBounds
    pub fn validate(&self, range: Range<u64>) -> core::Result<()> {
        Validate { range }.execute(self.connection_key, self.transfer_key)
    }

    /// Updates various properties on a placeholder.
    ///
    /// This method is equivalent to calling `Update::execute`.
    pub fn update(&self, options: UpdateOptions) -> core::Result<()> {
        options.0.execute(self.connection_key, self.transfer_key)
    }

    /// Shortcut for calling `Placeholder::update` with `UpdateOptions::mark_sync`.
    pub fn mark_sync(&self) -> core::Result<()> {
        self.update(UpdateOptions::new().mark_sync())
    }

    /// Shortcut for calling `Placeholder::update` with `UpdateOptions::metadata`.
    pub fn set_metadata(&self, metadata: Metadata) -> core::Result<()> {
        self.update(UpdateOptions::new().metadata(metadata))
    }

    /// Shortcut for calling `Placeholder::update` with `UpdateOptions::blob`.
    pub fn set_blob(self, blob: &[u8]) -> core::Result<()> {
        self.update(UpdateOptions::new().blob(blob))
    }

    /// Displays a progress bar next to the file in the file explorer to show the progress of the
    /// current operation. In addition, the standard Windows file progress dialog will open
    /// displaying the speed and progress based on the values set. During background hydrations,
    /// an interactive toast will appear notifying the user of an operation with a progress bar.
    // TODO: Add gifs to the docs to show each type of display
    pub fn set_progress(&self, total: u64, completed: u64) -> core::Result<()> {
        unsafe {
            CfReportProviderProgress(
                CF_CONNECTION_KEY(self.connection_key),
                self.transfer_key,
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

// TODO: does this have the same 4KiB requirement as writing?
impl io::Read for Placeholder {
    /// Read data from a placeholder file.
    ///
    /// This method is equivalent to calling `Read::execute`, except it will not increment the
    /// file cursor.
    ///
    /// The bytes returned will ALWAYS be the length of the buffer passed in. The operating
    /// system provides these guarantees.
    // TODO: as far as I know, what I said above is true, double check though
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let result = Read {
            buffer,
            position: self.position,
        }
        .execute(self.connection_key, self.transfer_key);

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
    /// Write data to a placeholder.
    ///
    /// The buffer passed must be 4KiB in length or end on the logical file size. Unfortunately,
    /// this is a restriction of the operating system. Read
    /// [here](https://github.com/ok-nick/wincs/issues/3) for a convenient abstraction.
    ///
    /// The bytes returned will ALWAYS be the length of the buffer passed in. The operating system
    /// provides these guarantees.
    ///
    /// This method is equivalent to calling `Write::execute`, except it will not increment the
    /// file cursor.
    // TODO: confirm the abovementioned is true
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        assert!(
            buffer.len() % 4096 == 0 || self.position + buffer.len() as u64 >= self.file_size,
            "the length of the buffer must be 4KiB aligned or ending on the logical file size"
        );

        let result = Write {
            buffer,
            position: self.position,
        }
        .execute(self.connection_key, self.transfer_key);

        match result {
            Ok(_) => {
                self.position += buffer.len() as u64;
                Ok(buffer.len())
            }
            Err(err) => Err(err.into()),
        }
    }

    /// This does not do anything.
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

/// Various properties to update a placeholder in batch.
#[derive(Debug, Clone, Default)]
pub struct UpdateOptions<'a>(Update<'a>);

impl<'a> UpdateOptions<'a> {
    /// Create a new `UpdateOptions`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks the placeholder as synced.
    ///
    /// A file or directory should be marked as "synced" when it has all of its data and metadata.
    /// A file that is partially full could still be marked as synced, any remaining data will
    /// invoke the `SyncFilter::fetch_data` callback automatically if requested.
    pub fn mark_sync(mut self) -> Self {
        self.0.mark_sync = true;
        self
    }

    /// The metadata for the placeholder.
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.0.metadata = Some(metadata);
        self
    }

    /// A buffer of bytes stored with the file that could be accessed through `Request::file_blob`
    /// or `FileExt::placeholder_info`.
    ///
    /// The buffer must not exceed
    /// [4KiB](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Storage/CloudFilters/constant.CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH.html).
    pub fn blob(mut self, blob: &'a [u8]) -> Self {
        assert!(
            blob.len() <= CloudFilters::CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH as usize,
            "blob size must not exceed {} byes, got {} bytes",
            CloudFilters::CF_PLACEHOLDER_MAX_FILE_IDENTITY_LENGTH,
            blob.len()
        );
        self.0.blob = Some(blob);
        self
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
