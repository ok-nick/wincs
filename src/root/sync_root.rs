use std::{ffi, mem::MaybeUninit, path::Path, ptr};

use widestring::{U16CString, U16Str, U16String};
use windows::{
    core::{self, HSTRING, PWSTR},
    Storage::Provider::StorageProviderSyncRootManager,
    Win32::{
        Foundation::{self, GetLastError, LocalFree, HANDLE, HLOCAL, SUCCESS},
        Security::{self, Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_USER},
        Storage::CloudFilters,
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

/// A builder to construct a [SyncRootId][crate::SyncRootId].
#[derive(Debug, Clone)]
pub struct SyncRootIdBuilder {
    provider_name: U16String,
    user_security_id: SecurityId,
    account_name: U16String,
}

impl SyncRootIdBuilder {
    /// Create a new builder with the given provider name.
    ///
    /// The provider name MUST NOT contain exclamation points and it must be within [255](https://docs.microsoft.com/en-us/windows/win32/api/cfapi/ns-cfapi-cf_sync_root_provider_info#remarks) characters.
    pub fn new(provider_name: U16String) -> Self {
        // TODO: assert that is doesn't have exclamation points
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

    /// The security id of the Windows user. Retrieve this value via the
    /// [SecurityId][crate::SecurityId] struct.
    ///
    /// By default, a sync root registered without a user security id will be installed globally.
    pub fn user_security_id(mut self, security_id: SecurityId) -> Self {
        self.user_security_id = security_id;
        self
    }

    /// The name of the user's account.
    ///
    /// This value does not have any actual meaning and it could theoretically be anything.
    /// However, it is encouraged to set this value to the account name of the user on the remote.
    pub fn account_name(mut self, account_name: U16String) -> Self {
        self.account_name = account_name;
        self
    }

    /// Constructs a [SyncRootId][crate::SyncRootId] from the builder.
    pub fn build(self) -> core::Result<SyncRootId> {
        Ok(SyncRootId(HSTRING::from_wide(
            &[
                self.provider_name.as_slice(),
                self.user_security_id.0.as_slice(),
                self.account_name.as_slice(),
            ]
            .join(&SyncRootId::SEPARATOR),
        )))
    }
}

/// The identifier for a sync root.
///
/// The inner value comes in the form:
/// `provider-id!security-id!account-name`
/// as specified
/// [here](https://docs.microsoft.com/en-us/uwp/api/windows.storage.provider.storageprovidersyncrootinfo.id?view=winrt-22000#property-value).
///
/// A [SyncRootId][crate::SyncRootId] stores an inner, reference counted [HSTRING][windows::core::HSTRING], making this struct cheap to clone.
#[derive(Debug, Clone)]
pub struct SyncRootId(HSTRING);

impl SyncRootId {
    // https://docs.microsoft.com/en-us/uwp/api/windows.storage.provider.storageprovidersyncrootinfo.id?view=winrt-22000#windows-storage-provider-storageprovidersyncrootinfo-id
    // unicode exclamation point as told in the specification above
    const SEPARATOR: u16 = 0x21;

    /// Creates a [SyncRootId][crate::SyncRootId] from the sync root at the given path.
    pub fn from_path<P: AsRef<Path>>(path: P) -> core::Result<Self> {
        // if the id is coming from a sync root, then it must be valid
        Ok(Self(path.as_ref().sync_root_info()?.Id()?))
    }

    /// Whether or not the [SyncRootId][crate::SyncRootId] has already been registered.
    pub fn is_registered(&self) -> core::Result<bool> {
        Ok(
            match StorageProviderSyncRootManager::GetSyncRootInformationForId(&self.0) {
                Ok(_) => true,
                Err(err) => err.code() != Foundation::ERROR_NOT_FOUND.to_hresult(),
            },
        )
    }

    /// Unregisters the sync root at the current [SyncRootId][crate::SyncRootId] if it exists.
    pub fn unregister(&self) -> core::Result<()> {
        StorageProviderSyncRootManager::Unregister(&self.0)
    }

    /// A reference to the [SyncRootId][crate::SyncRootId] as a 16 bit string.
    pub fn as_u16str(&self) -> &U16Str {
        U16Str::from_slice(&self.0)
    }

    /// A reference to the [SyncRootId][crate::SyncRootId] as an [HSTRING][windows::core::HSTRING] (its inner value).
    pub fn as_hstring(&self) -> &HSTRING {
        &self.0
    }

    /// The three components of a [SyncRootId][crate::SyncRootId] as described by the specification.
    ///
    /// The order goes as follows:
    /// `(provider-id, security-id, account-name)`
    pub fn to_components(&self) -> core::Result<(&U16Str, &U16Str, &U16Str)> {
        // Create an iterator that will yield a maximum of 3 parts.
        let mut parts = self.0.splitn(3, |&byte| byte == Self::SEPARATOR);

        // Pattern match on the iterator to safely extract the three parts.
        if let (Some(first), Some(second), Some(third)) = (parts.next(), parts.next(), parts.next())
        {
            Ok((
                U16Str::from_slice(first),
                U16Str::from_slice(second),
                U16Str::from_slice(third),
            ))
        } else {
            Err(Foundation::ERROR_INVALID_DATA.into())
        }
    }
}

/// A user security id (SID).
#[derive(Debug, Clone)]
pub struct SecurityId(U16String);

impl SecurityId {
    // https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentthreadeffectivetoken
    const CURRENT_THREAD_EFFECTIVE_TOKEN: HANDLE = HANDLE(-6i32 as *mut ffi::c_void);

    /// Creates a new [SecurityId][crate::SecurityId] without any assertions.
    pub fn new_unchecked(id: U16String) -> Self {
        Self(id)
    }

    /// The [SecurityId][crate::SecurityId] for the logged in user.
    pub fn current_user() -> core::Result<Self> {
        unsafe {
            let mut token_size = 0;
            let mut token = MaybeUninit::<TOKEN_USER>::uninit();

            if GetTokenInformation(
                Self::CURRENT_THREAD_EFFECTIVE_TOKEN,
                Security::TokenUser,
                None,
                0,
                &mut token_size,
            )
            .is_err()
                && GetLastError() == Foundation::ERROR_INSUFFICIENT_BUFFER
            {
                GetTokenInformation(
                    Self::CURRENT_THREAD_EFFECTIVE_TOKEN,
                    Security::TokenUser,
                    Some(&mut token as *mut _ as *mut _),
                    token_size,
                    &mut token_size,
                )?;
            }

            let token = token.assume_init();
            let mut sid = PWSTR(ptr::null_mut());
            ConvertSidToStringSidW(token.User.Sid, &mut sid as *mut _)?;

            let string_sid = U16CString::from_ptr_str(sid.0).into_ustring();

            // Fix the LocalFree call - we should handle the result properly
            if !LocalFree(Some(HLOCAL(sid.0 as *mut _))).0.is_null() {
                let last_error = GetLastError();
                if last_error.0 != SUCCESS {
                    return Err(last_error.into());
                }
            }

            Ok(SecurityId::new_unchecked(string_sid))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syncroot_id_parse() {
        let id = SyncRootId(HSTRING::from("provider-id!security-id!account-name"));
        let components = id.to_components();
        assert!(components.is_ok());

        let (provider, security, account) = id.to_components().unwrap();
        assert_eq!(provider, U16String::from("provider-id"));
        assert_eq!(security, U16String::from("security-id"));
        assert_eq!(account, U16String::from("account-name"));
    }

    #[test]
    fn test_invalid_syncroot_id_parse() {
        let id = SyncRootId(HSTRING::from("provider-id!security-id0000"));
        let components = id.to_components();
        assert!(components.is_err());
    }

    #[test]
    fn test_empty_syncroot_id_parse() {
        let id = SyncRootId(HSTRING::from(""));
        let components = id.to_components();
        assert!(components.is_err());
    }
}
