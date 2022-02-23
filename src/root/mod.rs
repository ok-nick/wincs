pub mod connect;
pub mod register;

use std::{
    mem::MaybeUninit,
    ops::{BitAndAssign, BitOrAssign, Not},
    path::{Path, PathBuf},
    ptr,
};

use widestring::{U16CStr, U16Str, U16String};
use windows::{
    core::{self, HSTRING},
    Foundation::Uri,
    Storage::{
        Provider::{
            StorageProviderHardlinkPolicy, StorageProviderSyncRootInfo,
            StorageProviderSyncRootManager,
        },
        StorageFolder,
        Streams::DataWriter,
    },
    Win32::{
        Foundation::{self, GetLastError, HANDLE, PWSTR},
        Security::{self, Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_USER},
        System::Memory::LocalFree,
    },
};

use crate::root::register::RegisterOptions;

// TODO: borrow all these fields
#[derive(Debug, Clone, Default)]
pub struct SyncRoot {
    provider_name: U16String,
    security_id: Option<U16String>,
    account_name: U16String,
}

impl SyncRoot {
    // TODO: these global functions should be moved to separate individual functions
    pub fn all() {
        // GetCurrentSyncRoots()
    }

    pub fn is_supported() -> core::Result<bool> {
        // TODO: This method is only supported on certain windows versions, is it
        // possible to check for support for before this?
        StorageProviderSyncRootManager::IsSupported()
    }

    // https://docs.microsoft.com/en-us/windows/win32/api/cfapi/ns-cfapi-cf_sync_registration
    // fields have a max length of 255 bytes (there is a constant with the value)
    pub fn new(provider_name: U16String, account_name: U16String) -> Self {
        Self {
            provider_name,
            security_id: None,
            account_name,
        }
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> core::Result<Self> {
        let info = info_from_path(path.as_ref())?;

        // TODO: don't convert it to a string and instead work with 16bit strings
        let id = info.Id()?.to_string_lossy();
        let id: Vec<&str> = id.split('!').collect();
        Ok(Self::new(id[0].into(), id[2].into()))
    }

    #[must_use]
    pub fn user_security_id(mut self, security_id: U16String) -> Self {
        self.security_id = Some(security_id);
        self
    }

    pub fn is_registered(&self) -> core::Result<bool> {
        let security_id = SecurityId::current()?;
        Ok(
            match StorageProviderSyncRootManager::GetSyncRootInformationForId(
                hstring_from_widestring(&sync_root_id(
                    &self.provider_name,
                    match &self.security_id {
                        Some(security_id) => security_id,
                        None => security_id.0.as_ustr(),
                    },
                    &self.account_name,
                )),
            ) {
                Ok(_) => true,
                Err(err) => err.win32_error() != Some(Foundation::ERROR_NOT_FOUND),
            },
        )
    }

    pub fn register<P: AsRef<Path>>(&self, path: P, options: RegisterOptions) -> core::Result<()> {
        let info = StorageProviderSyncRootInfo::new()?;

        info.SetProtectionMode(options.protection_mode.into())?;
        info.SetShowSiblingsAsGroup(options.show_siblings_as_group)?;
        info.SetHydrationPolicy(options.hydration_type.into())?;
        info.SetHydrationPolicyModifier(options.hydration_policy.0)?;
        info.SetPopulationPolicy(options.population_type.into())?;
        info.SetInSyncPolicy(options.in_sync_policy.0)?;
        info.SetDisplayNameResource(hstring_from_widestring(
            options
                .display_name
                .as_deref()
                .unwrap_or(&self.provider_name),
        ))?;
        info.SetPath(
            StorageFolder::GetFolderFromPathAsync(hstring_from_widestring(
                &U16String::from_os_str(path.as_ref().as_os_str()),
            ))?
            .get()?,
        )?;
        info.SetHardlinkPolicy(if options.allow_hardlinks {
            StorageProviderHardlinkPolicy::Allowed
        } else {
            StorageProviderHardlinkPolicy::None
        })?;
        // TODO: Avoid allocating a security id unless if `options.security_id` is `None`
        let security_id = SecurityId::current()?;
        info.SetId(hstring_from_widestring(&sync_root_id(
            &self.provider_name,
            match &self.security_id {
                Some(security_id) => security_id,
                None => security_id.0.as_ustr(),
                // None => SecurityId::current()?.0.as_ustr(),
            },
            &self.account_name,
        )))?;

        if let Some(provider_id) = options.provider_id {
            info.SetProviderId(provider_id)?;
        }
        if let Some(version) = &options.version {
            info.SetVersion(hstring_from_widestring(version))?;
        }
        if let Some(icon_path) = &options.icon_path {
            info.SetIconResource(hstring_from_widestring(icon_path))?;
        }
        if let Some(uri) = &options.recycle_bin_uri {
            info.SetRecycleBinUri(Uri::CreateUri(hstring_from_widestring(uri))?)?;
        }
        if let Some(blob) = &options.blob {
            // TODO: implement IBuffer interface for slices to avoid a copy
            let writer = DataWriter::new()?;
            writer.WriteBytes(blob)?;
            info.SetContext(writer.DetachBuffer()?)?;
        }

        StorageProviderSyncRootManager::Register(info)
    }

    pub fn unregister(&self) -> core::Result<()> {
        // TODO: Avoid allocating a security id unless if `self.security_id` is `None`
        let security_id = SecurityId::current()?;
        StorageProviderSyncRootManager::Unregister(hstring_from_widestring(&sync_root_id(
            &self.provider_name,
            match &self.security_id {
                Some(security_id) => security_id,
                None => security_id.0.as_ustr(),
            },
            &self.account_name,
        )))
    }
}

pub trait PathExt {
    // TODO: if `sync_root_info` doesn't error then this is true
    fn is_registered(&self) -> bool {
        todo!()
    }

    // TODO: uses `info_from_path`. This call requires a struct to be made for getters of StorageProviderSyncRootInfo
    fn sync_root_info(&self) {
        todo!()
    }
}

impl PathExt for PathBuf {}
impl PathExt for Path {}

#[derive(Debug, Clone)]
pub struct SecurityId(pub(crate) &'static U16CStr);

impl SecurityId {
    // Equivalent to the return of `GetCurrentThreadEffectiveToken`
    const CURRENT_THREAD_EFFECTIVE_TOKEN: HANDLE = HANDLE(-6);

    pub fn current() -> core::Result<Self> {
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
            ConvertSidToStringSidW(token.User.Sid, &mut sid).ok()?;

            Ok(Self(U16CStr::from_ptr_str(sid.0)))
        }
    }
}

impl Drop for SecurityId {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.0.as_ptr() as isize);
        }
    }
}

pub fn sync_root_id(
    provider_name: &U16Str,
    security_id: &U16Str,
    account_name: &U16Str,
) -> U16String {
    let mut id = U16String::with_capacity(
        provider_name.len() + 1 + security_id.len() + 1 + account_name.len(),
    );
    id.push(provider_name);
    id.push_char('!');
    id.push(security_id);
    id.push_char('!');
    id.push(account_name);
    id
}

pub fn info_from_path(path: &Path) -> core::Result<StorageProviderSyncRootInfo> {
    StorageProviderSyncRootManager::GetSyncRootInformationForFolder(
        StorageFolder::GetFolderFromPathAsync(hstring_from_widestring(&U16String::from_os_str(
            path.as_os_str(),
        )))?
        .get()?,
    )
}

// TODO: Move this somewhere more global
pub fn hstring_from_widestring<T: AsRef<[u16]>>(bytes: T) -> HSTRING {
    HSTRING::from_wide(bytes.as_ref())
}

// same here
pub fn set_flag<T>(flags: &mut T, flag: T, yes: bool)
where
    T: BitOrAssign + BitAndAssign + Not<Output = T>,
{
    if yes {
        *flags |= flag;
    } else {
        *flags &= !flag;
    }
}
