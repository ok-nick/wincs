use std::path::Path;

use widestring::{U16Str, U16String};
use windows::{
    core::{self, GUID},
    Foundation::Uri,
    Storage::{
        Provider::{
            StorageProviderHardlinkPolicy, StorageProviderHydrationPolicy,
            StorageProviderHydrationPolicyModifier, StorageProviderInSyncPolicy,
            StorageProviderPopulationPolicy, StorageProviderProtectionMode,
            StorageProviderSyncRootInfo, StorageProviderSyncRootManager,
        },
        StorageFolder,
        Streams::DataWriter,
    },
    Win32::Storage::CloudFilters::{
        self, CF_HYDRATION_POLICY_MODIFIER_USHORT, CF_HYDRATION_POLICY_PRIMARY,
        CF_HYDRATION_POLICY_PRIMARY_USHORT, CF_INSYNC_POLICY, CF_POPULATION_POLICY_PRIMARY,
        CF_POPULATION_POLICY_PRIMARY_USHORT,
    },
};

use crate::utility::ToHString;

use super::SyncRootId;

#[derive(Debug, Clone)]
pub struct Registration<'a> {
    sync_root_id: &'a SyncRootId,
    show_siblings_as_group: bool,
    allow_pinning: bool,
    allow_hardlinks: bool,
    display_name: &'a U16Str,
    recycle_bin_uri: Option<&'a U16Str>,
    version: Option<&'a U16Str>,
    hydration_type: HydrationType,
    hydration_policy: HydrationPolicy,
    population_type: PopulationType,
    protection_mode: ProtectionMode,
    provider_id: Option<GUID>,
    supported_attributes: SupportedAttributes,
    icon: U16String,
    blob: Option<&'a [u8]>,
}

impl<'a> Registration<'a> {
    pub fn from_sync_root_id(sync_root_id: &'a SyncRootId) -> Self {
        Self {
            sync_root_id,
            display_name: sync_root_id.as_u16str(),
            recycle_bin_uri: None,
            show_siblings_as_group: false,
            allow_pinning: false,
            version: None,
            provider_id: None,
            protection_mode: ProtectionMode::Unknown,
            allow_hardlinks: false,
            hydration_type: HydrationType::Progressive, // stated as default in the docs
            hydration_policy: HydrationPolicy::default(),
            population_type: PopulationType::Full,
            supported_attributes: SupportedAttributes::default(),
            icon: U16String::from_str("C:\\Windows\\System32\\imageres.dll,1525"),
            blob: None,
        }
    }

    pub fn hydration_type(mut self, hydration_type: HydrationType) -> Self {
        self.hydration_type = hydration_type;
        self
    }

    pub fn allow_pinning(mut self) -> Self {
        self.allow_pinning = true;
        self
    }

    pub fn allow_hardlinks(mut self) -> Self {
        self.allow_hardlinks = true;
        self
    }

    // This field is required

    pub fn display_name(mut self, display_name: &'a U16Str) -> Self {
        self.display_name = display_name;
        self
    }

    pub fn recycle_bin_uri(mut self, uri: &'a U16Str) -> Self {
        self.recycle_bin_uri = Some(uri);
        self
    }

    // I think this is for sync roots with the same provider name?

    pub fn show_siblings_as_group(mut self) -> Self {
        self.show_siblings_as_group = true;
        self
    }

    pub fn population_type(mut self, population_type: PopulationType) -> Self {
        self.population_type = population_type;
        self
    }

    pub fn version(mut self, version: &'a U16Str) -> Self {
        assert!(
            version.len() <= CloudFilters::CF_MAX_PROVIDER_VERSION_LENGTH as usize,
            "version length must not exceed {} characters, got {} characters",
            CloudFilters::CF_MAX_PROVIDER_VERSION_LENGTH,
            version.len()
        );
        self.version = Some(version);
        self
    }

    pub fn protection_mode(mut self, protection_mode: ProtectionMode) -> Self {
        self.protection_mode = protection_mode;
        self
    }

    pub fn supported_attributes(mut self, supported_attributes: SupportedAttributes) -> Self {
        self.supported_attributes = supported_attributes;
        self
    }

    pub fn hydration_policy(mut self, hydration_policy: HydrationPolicy) -> Self {
        self.hydration_policy = hydration_policy;
        self
    }

    // TODO: this field is required
    // https://docs.microsoft.com/en-us/windows/win32/menurc/icon-resource

    pub fn icon(mut self, mut path: U16String, index: u16) -> Self {
        path.push_str(format!(",{index}"));
        self.icon = path;
        self
    }

    pub fn blob(mut self, blob: &'a [u8]) -> Self {
        assert!(
            blob.len() <= 65536,
            "blob size must not exceed 65536 bytes, got {} bytes",
            blob.len()
        );
        self.blob = Some(blob);
        self
    }

    pub fn register<P: AsRef<Path>>(&self, path: P) -> core::Result<()> {
        let info = StorageProviderSyncRootInfo::new()?;

        info.SetProtectionMode(self.protection_mode.into())?;
        info.SetShowSiblingsAsGroup(self.show_siblings_as_group)?;
        info.SetHydrationPolicy(self.hydration_type.into())?;
        info.SetHydrationPolicyModifier(self.hydration_policy.0)?;
        info.SetPopulationPolicy(self.population_type.into())?;
        info.SetInSyncPolicy(self.supported_attributes.0)?;
        info.SetDisplayNameResource(self.display_name.to_hstring())?;
        info.SetIconResource(self.icon.to_hstring())?;
        info.SetPath(
            StorageFolder::GetFolderFromPathAsync(
                &U16String::from_os_str(path.as_ref().as_os_str()).to_hstring(),
            )?
            .get()?,
        )?;
        info.SetHardlinkPolicy(if self.allow_hardlinks {
            StorageProviderHardlinkPolicy::Allowed
        } else {
            StorageProviderHardlinkPolicy::None
        })?;
        info.SetId(self.sync_root_id.as_hstring())?;

        if let Some(provider_id) = self.provider_id {
            info.SetProviderId(provider_id)?;
        }
        if let Some(version) = &self.version {
            info.SetVersion(version.to_hstring())?;
        }

        if let Some(uri) = &self.recycle_bin_uri {
            info.SetRecycleBinUri(Uri::CreateUri(uri.to_hstring())?)?;
        }
        if let Some(blob) = &self.blob {
            // TODO: implement IBuffer interface for slices to avoid a copy
            let writer = DataWriter::new()?;
            writer.WriteBytes(blob)?;
            info.SetContext(writer.DetachBuffer()?)?;
        }

        StorageProviderSyncRootManager::Register(info)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProtectionMode {
    Personal,
    Unknown,
}

impl From<ProtectionMode> for StorageProviderProtectionMode {
    fn from(mode: ProtectionMode) -> Self {
        match mode {
            ProtectionMode::Personal => StorageProviderProtectionMode::Personal,
            ProtectionMode::Unknown => StorageProviderProtectionMode::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HydrationType {
    Partial,
    Progressive,
    Full,
    AlwaysFull,
}

impl From<HydrationType> for StorageProviderHydrationPolicy {
    fn from(hydration_type: HydrationType) -> Self {
        match hydration_type {
            HydrationType::Partial => StorageProviderHydrationPolicy::Partial,
            HydrationType::Progressive => StorageProviderHydrationPolicy::Progressive,
            HydrationType::Full => StorageProviderHydrationPolicy::Full,
            HydrationType::AlwaysFull => StorageProviderHydrationPolicy::AlwaysFull,
        }
    }
}

impl From<CF_HYDRATION_POLICY_PRIMARY_USHORT> for HydrationType {
    fn from(primary: CF_HYDRATION_POLICY_PRIMARY_USHORT) -> Self {
        match CF_HYDRATION_POLICY_PRIMARY(primary.us) {
            CloudFilters::CF_HYDRATION_POLICY_PARTIAL => HydrationType::Partial,
            CloudFilters::CF_HYDRATION_POLICY_PROGRESSIVE => HydrationType::Progressive,
            CloudFilters::CF_HYDRATION_POLICY_FULL => HydrationType::Full,
            CloudFilters::CF_HYDRATION_POLICY_ALWAYS_FULL => HydrationType::AlwaysFull,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HydrationPolicy(pub(crate) StorageProviderHydrationPolicyModifier);

impl HydrationPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn require_validation(mut self) -> Self {
        self.0 |= StorageProviderHydrationPolicyModifier::ValidationRequired;
        self
    }

    // TODO: assert this, it is incompatible with the validation required parameter
    // https://docs.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_hydration_policy_modifier

    pub fn allow_streaming(mut self) -> Self {
        self.0 |= StorageProviderHydrationPolicyModifier::StreamingAllowed;
        self
    }

    pub fn allow_platform_dehydration(mut self) -> Self {
        self.0 |= StorageProviderHydrationPolicyModifier::AutoDehydrationAllowed;
        self
    }

    pub fn allow_full_restart_hydration(mut self) -> Self {
        self.0 |= StorageProviderHydrationPolicyModifier::AllowFullRestartHydration;
        self
    }
}

impl Default for HydrationPolicy {
    fn default() -> Self {
        Self(StorageProviderHydrationPolicyModifier::None)
    }
}

impl From<CF_HYDRATION_POLICY_MODIFIER_USHORT> for HydrationPolicy {
    fn from(primary: CF_HYDRATION_POLICY_MODIFIER_USHORT) -> Self {
        Self(StorageProviderHydrationPolicyModifier(primary.us as u32))
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PopulationType {
    Full,
    AlwaysFull,
}

impl From<PopulationType> for StorageProviderPopulationPolicy {
    fn from(population_type: PopulationType) -> StorageProviderPopulationPolicy {
        match population_type {
            PopulationType::Full => StorageProviderPopulationPolicy::Full,
            PopulationType::AlwaysFull => StorageProviderPopulationPolicy::AlwaysFull,
        }
    }
}

impl From<CF_POPULATION_POLICY_PRIMARY_USHORT> for PopulationType {
    fn from(primary: CF_POPULATION_POLICY_PRIMARY_USHORT) -> Self {
        match CF_POPULATION_POLICY_PRIMARY(primary.us) {
            CloudFilters::CF_POPULATION_POLICY_FULL => PopulationType::Full,
            CloudFilters::CF_POPULATION_POLICY_ALWAYS_FULL => PopulationType::AlwaysFull,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SupportedAttributes(pub(crate) StorageProviderInSyncPolicy);

impl SupportedAttributes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn file_creation_time(mut self) -> Self {
        self.0 |= StorageProviderInSyncPolicy::FileCreationTime;
        self
    }

    pub fn file_readonly(mut self) -> Self {
        self.0 |= StorageProviderInSyncPolicy::FileReadOnlyAttribute;
        self
    }

    pub fn file_hidden(mut self) -> Self {
        self.0 |= StorageProviderInSyncPolicy::FileHiddenAttribute;
        self
    }

    pub fn file_system(mut self) -> Self {
        self.0 |= StorageProviderInSyncPolicy::FileSystemAttribute;
        self
    }

    pub fn file_last_write_time(mut self) -> Self {
        self.0 |= StorageProviderInSyncPolicy::FileLastWriteTime;
        self
    }

    pub fn directory_creation_time(mut self) -> Self {
        self.0 |= StorageProviderInSyncPolicy::DirectoryCreationTime;
        self
    }

    pub fn directory_readonly(mut self) -> Self {
        self.0 |= StorageProviderInSyncPolicy::DirectoryReadOnlyAttribute;
        self
    }

    pub fn directory_hidden(mut self) -> Self {
        self.0 |= StorageProviderInSyncPolicy::DirectoryHiddenAttribute;
        self
    }

    pub fn directory_last_write_time(mut self) -> Self {
        self.0 |= StorageProviderInSyncPolicy::DirectoryLastWriteTime;
        self
    }

    // TODO: I'm not sure how this differs from the default policy,
    // https://docs.microsoft.com/en-us/answers/questions/760677/how-does-cf-insync-policy-none-differ-from-cf-insy.html

    pub fn none(mut self) -> Self {
        self.0 |= StorageProviderInSyncPolicy::PreserveInsyncForSyncEngine;
        self
    }
}

impl Default for SupportedAttributes {
    fn default() -> Self {
        Self(StorageProviderInSyncPolicy::Default)
    }
}

impl From<CF_INSYNC_POLICY> for SupportedAttributes {
    fn from(policy: CF_INSYNC_POLICY) -> Self {
        Self(StorageProviderInSyncPolicy(policy.0))
    }
}
