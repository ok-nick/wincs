use std::{mem::MaybeUninit, path::Path, ptr};

use widestring::{U16CString, U16Str, U16String};
use windows::{
    core::{self, HSTRING, PWSTR},
    Storage::Provider::StorageProviderSyncRootManager,
    Win32::{
        Foundation::{self, GetLastError, HANDLE},
        Security::{self, Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_USER},
        Storage::CloudFilters,
        System::Memory::LocalFree,
    },
};

use crate::ext::PathExt;

/// Returns a list of active sync roots.
pub fn active_roots() {
    // GetCurrentSyncRoots()
    todo!()
}

/// Returns whether or not the Cloud Filter API is supported (or at least the UWP part of it, for
/// now).
pub fn is_supported() -> core::Result<bool> {
    // TODO: Check current windows version to see if this function is supported before calling it
    StorageProviderSyncRootManager::IsSupported()
}

#[derive(Debug, Clone)]
pub struct SyncRootBuilder {
    provider_name: U16String,
    user_security_id: SecurityId,
    account_name: U16String,
}

impl SyncRootBuilder {
    pub fn new(provider_name: U16String) -> Self {
        assert!(
            provider_name.len() <= CloudFilters::CF_MAX_PROVIDER_NAME_LENGTH as usize,
            "provider name must not exceed {} characters, got {} characters",
            CloudFilters::CF_MAX_PROVIDER_NAME_LENGTH,
            provider_name.len()
        );

        Self {
            provider_name,
            user_security_id: SecurityId(U16String::new()),
            account_name: U16String::new(),
        }
    }

    pub fn user_security_id(&mut self, security_id: SecurityId) -> &mut Self {
        self.user_security_id = security_id;
        self
    }

    pub fn account_name(&mut self, account_name: U16String) -> &mut Self {
        self.account_name = account_name;
        self
    }

    pub fn build(self) -> SyncRoot {
        SyncRoot {
            provider_name: self.provider_name,
            user_security_id: self.user_security_id,
            account_name: self.account_name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SyncRoot {
    provider_name: U16String,
    user_security_id: SecurityId,
    account_name: U16String,
}

impl SyncRoot {
    pub fn builder(provider_name: U16String) -> SyncRootBuilder {
        SyncRootBuilder::new(provider_name)
    }

    pub fn provider_name(&self) -> &U16Str {
        &self.provider_name
    }

    pub fn user_security_id(&self) -> &SecurityId {
        &self.user_security_id
    }

    pub fn account_name(&self) -> &U16Str {
        &self.account_name
    }

    pub fn to_id(&self) -> SyncRootId {
        SyncRootId(HSTRING::from_wide(
            &[
                self.provider_name.as_slice(),
                self.user_security_id.0.as_slice(),
                self.account_name.as_slice(),
            ]
            .join(&SyncRootId::SEPARATOR),
        ))
    }
}

// an HSTRING is reference counted, it is safe to clone
#[derive(Debug, Clone)]
pub struct SyncRootId(HSTRING);

impl SyncRootId {
    // https://docs.microsoft.com/en-us/uwp/api/windows.storage.provider.storageprovidersyncrootinfo.id?view=winrt-22000#windows-storage-provider-storageprovidersyncrootinfo-id
    // unicode exclamation point as told in the specification above
    const SEPARATOR: u16 = 0x21;

    // the extra check doesn't always mean the id is valid, if exclamation points are misplaced
    // then an error would occur down the line
    pub fn from_path<P: AsRef<Path>>(path: P) -> core::Result<Self> {
        let id = path.as_ref().sync_root_info()?.Id()?;

        // TODO: A valid ID would have more restrictions, like 255 (+ null) limit for provider-id,
        // and 255 (+ null) limit for security ids
        let excl_points = id
            .as_wide()
            .iter()
            .filter(|&&byte| byte == Self::SEPARATOR)
            .count();
        assert!(
            excl_points >= 2,
            "malformed sync root id, missing {} component(s)",
            excl_points + 1
        );

        Self::from_path_unchecked(path)
    }

    pub fn from_path_unchecked<P: AsRef<Path>>(path: P) -> core::Result<Self> {
        Ok(Self(path.as_ref().sync_root_info()?.Id()?))
    }

    pub fn is_registered(&self) -> core::Result<bool> {
        Ok(
            match StorageProviderSyncRootManager::GetSyncRootInformationForId(&self.0) {
                Ok(_) => true,
                Err(err) => err.win32_error() != Some(Foundation::ERROR_NOT_FOUND),
            },
        )
    }

    pub fn unregister(&self) -> core::Result<()> {
        StorageProviderSyncRootManager::Unregister(&self.0)
    }

    pub fn as_u16str(&self) -> &U16Str {
        U16Str::from_slice(self.0.as_wide())
    }

    // splits up a sync root id into its three components according to the specification,
    // https://docs.microsoft.com/en-us/uwp/api/windows.storage.provider.storageprovidersyncrootinfo.id?view=winrt-22000#windows-storage-provider-storageprovidersyncrootinfo-id
    // provider-id!security-id!account-name
    // TODO: This doesn't work properly, it forgets to include the account name
    pub fn to_components(&self) -> (&U16Str, &U16Str, &U16Str) {
        let mut components = Vec::with_capacity(3);
        let mut bytes = self.0.as_wide();

        for index in 0..2 {
            match bytes.iter().position(|&byte| byte == Self::SEPARATOR) {
                Some(position) => {
                    components.insert(index, U16Str::from_slice(bytes));
                    bytes = &bytes[(position + 1)..];
                }
                None => {
                    // TODO: return a result instead of panic
                    panic!("malformed sync root id, got {:?}", components)
                }
            }
        }

        (components[0], components[1], components[2])
    }

    pub fn to_sync_root(&self) -> SyncRoot {
        let id = self.to_components();
        SyncRoot {
            provider_name: id.0.to_owned(),
            user_security_id: SecurityId::new_unchecked(id.1.to_owned()),
            account_name: id.2.to_owned(),
        }
    }

    pub(crate) fn into_inner(self) -> HSTRING {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct SecurityId(U16String);

impl SecurityId {
    // https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentthreadeffectivetoken
    const CURRENT_THREAD_EFFECTIVE_TOKEN: HANDLE = HANDLE(-6);

    pub fn new_unchecked(id: U16String) -> Self {
        Self(id)
    }

    pub fn current_user() -> core::Result<Self> {
        unsafe {
            let mut token_size = 0;
            let mut token = MaybeUninit::<TOKEN_USER>::uninit();

            if !GetTokenInformation(
                Self::CURRENT_THREAD_EFFECTIVE_TOKEN,
                Security::TokenUser,
                ptr::null_mut(),
                0,
                &mut token_size,
            )
            .as_bool()
                && GetLastError() == Foundation::ERROR_INSUFFICIENT_BUFFER
            {
                GetTokenInformation(
                    Self::CURRENT_THREAD_EFFECTIVE_TOKEN,
                    Security::TokenUser,
                    &mut token as *mut _ as *mut _,
                    token_size,
                    &mut token_size,
                )
                .ok()?;
            }

            let token = token.assume_init();
            let mut sid = PWSTR(ptr::null_mut());
            ConvertSidToStringSidW(token.User.Sid, &mut sid as *mut _).ok()?;

            let string_sid = U16CString::from_ptr_str(sid.0).into_ustring();
            LocalFree(sid.0 as isize);

            Ok(SecurityId::new_unchecked(string_sid))
        }
    }
}
