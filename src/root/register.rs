use widestring::U16String;
use windows::{
    core::GUID,
    Storage::Provider::{
        StorageProviderHydrationPolicy, StorageProviderHydrationPolicyModifier,
        StorageProviderInSyncPolicy, StorageProviderPopulationPolicy,
        StorageProviderProtectionMode,
    },
    Win32::Storage::CloudFilters::{
        self, CF_HYDRATION_POLICY_MODIFIER_USHORT, CF_HYDRATION_POLICY_PRIMARY,
        CF_HYDRATION_POLICY_PRIMARY_USHORT, CF_INSYNC_POLICY, CF_POPULATION_POLICY_PRIMARY,
        CF_POPULATION_POLICY_PRIMARY_USHORT,
    },
};

use crate::root::set_flag;

#[derive(Debug, Clone)]
pub struct RegisterOptions<'a> {
    pub(crate) show_siblings_as_group: bool,
    pub(crate) allow_pinning: bool,
    pub(crate) allow_hardlinks: bool,
    pub(crate) display_name: Option<U16String>,
    pub(crate) recycle_bin_uri: Option<U16String>,
    pub(crate) version: Option<U16String>,
    pub(crate) hydration_type: HydrationType,
    pub(crate) hydration_policy: HydrationPolicy,
    pub(crate) population_type: PopulationType,
    pub(crate) protection_mode: ProtectionMode,
    pub(crate) provider_id: Option<GUID>,
    pub(crate) in_sync_policy: InSyncPolicy,
    pub(crate) icon_path: Option<U16String>,
    pub(crate) blob: Option<&'a [u8]>,
}

impl<'a> RegisterOptions<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn hydration_type(mut self, hydration_type: HydrationType) -> Self {
        self.hydration_type = hydration_type;
        self
    }

    #[must_use]
    pub fn allow_pinning(mut self, yes: bool) -> Self {
        self.allow_pinning = yes;
        self
    }

    #[must_use]
    pub fn allow_hardlinks(mut self, yes: bool) -> Self {
        self.allow_hardlinks = yes;
        self
    }

    // I made this default to the provider_name
    #[must_use]
    pub fn display_name(mut self, display_name: U16String) -> Self {
        self.display_name = Some(display_name);
        self
    }

    #[must_use]
    pub fn recycle_bin_uri(mut self, uri: U16String) -> Self {
        self.recycle_bin_uri = Some(uri);
        self
    }

    #[must_use]
    pub fn show_siblings_as_group(mut self, yes: bool) -> Self {
        self.show_siblings_as_group = yes;
        self
    }

    #[must_use]
    pub fn population_type(mut self, population_type: PopulationType) -> Self {
        self.population_type = population_type;
        self
    }

    #[must_use]
    pub fn version(mut self, version: U16String) -> Self {
        assert!(
            version.len() <= CloudFilters::CF_MAX_PROVIDER_VERSION_LENGTH as usize,
            "version length must not exceed {} characters, got {} characters",
            CloudFilters::CF_MAX_PROVIDER_VERSION_LENGTH,
            version.len()
        );
        self.version = Some(version);
        self
    }

    #[must_use]
    pub fn protection_mode(mut self, protection_mode: ProtectionMode) -> Self {
        self.protection_mode = protection_mode;
        self
    }

    #[must_use]
    pub fn in_sync_policy(mut self, in_sync_policy: InSyncPolicy) -> Self {
        self.in_sync_policy = in_sync_policy;
        self
    }

    #[must_use]
    pub fn hydration_policy(mut self, hydration_policy: HydrationPolicy) -> Self {
        self.hydration_policy = hydration_policy;
        self
    }

    // TODO: This is a bundled resource path, so like "C:\\blah\\boo.exe,0"
    // Maybe instead I should take parameters, "path, index," something like that
    #[must_use]
    pub fn icon_path(mut self, icon_path: U16String) -> Self {
        self.icon_path = Some(icon_path);
        self
    }

    #[must_use]
    pub fn blob(mut self, blob: &'a [u8]) -> Self {
        assert!(
            blob.len() <= 65536,
            "blob size must not exceed 64KB (65536 bytes) after serialization, got {} bytes",
            blob.len()
        );
        self.blob = Some(blob);
        self
    }
}

impl Default for RegisterOptions<'_> {
    fn default() -> Self {
        Self {
            display_name: None,
            recycle_bin_uri: None,
            show_siblings_as_group: false,
            allow_pinning: true,
            version: None,
            provider_id: None,
            protection_mode: ProtectionMode::Unknown,
            allow_hardlinks: true,
            hydration_type: HydrationType::Progressive, // stated as the default in the docs
            hydration_policy: HydrationPolicy::default(),
            population_type: PopulationType::Full,
            in_sync_policy: InSyncPolicy::default(),
            icon_path: None,
            blob: None,
        }
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

    #[must_use]
    pub fn validation_required(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderHydrationPolicyModifier::ValidationRequired,
            yes,
        );
        self
    }

    #[must_use]
    pub fn streaming_allowed(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderHydrationPolicyModifier::StreamingAllowed,
            yes,
        );
        self
    }

    #[must_use]
    pub fn auto_dehydration_allowed(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderHydrationPolicyModifier::AutoDehydrationAllowed,
            yes,
        );
        self
    }

    #[must_use]
    pub fn allow_full_restart_hydration(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderHydrationPolicyModifier::AllowFullRestartHydration,
            yes,
        );
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
pub struct InSyncPolicy(pub(crate) StorageProviderInSyncPolicy);

impl InSyncPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn file_creation_time(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderInSyncPolicy::FileCreationTime,
            yes,
        );
        self
    }

    #[must_use]
    pub fn file_readonly_attribute(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderInSyncPolicy::FileReadOnlyAttribute,
            yes,
        );
        self
    }

    #[must_use]
    pub fn hidden_attribute(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderInSyncPolicy::FileHiddenAttribute,
            yes,
        );
        self
    }

    #[must_use]
    pub fn file_system_attribute(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderInSyncPolicy::FileSystemAttribute,
            yes,
        );
        self
    }

    #[must_use]
    pub fn directory_creation_time(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderInSyncPolicy::DirectoryCreationTime,
            yes,
        );
        self
    }

    #[must_use]
    pub fn directory_readonly_attribute(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderInSyncPolicy::DirectoryReadOnlyAttribute,
            yes,
        );
        self
    }

    #[must_use]
    pub fn directory_hidden_attribute(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderInSyncPolicy::DirectoryHiddenAttribute,
            yes,
        );
        self
    }

    #[must_use]
    pub fn directory_last_write_time(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderInSyncPolicy::DirectoryLastWriteTime,
            yes,
        );
        self
    }

    #[must_use]

    pub fn file_last_write_time(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderInSyncPolicy::FileLastWriteTime,
            yes,
        );
        self
    }

    #[must_use]
    pub fn preserve_insync_for_sync_engine(mut self, yes: bool) -> Self {
        set_flag(
            &mut self.0,
            StorageProviderInSyncPolicy::PreserveInsyncForSyncEngine,
            yes,
        );
        self
    }
}

impl Default for InSyncPolicy {
    fn default() -> Self {
        Self(StorageProviderInSyncPolicy::Default)
    }
}

impl From<CF_INSYNC_POLICY> for InSyncPolicy {
    fn from(policy: CF_INSYNC_POLICY) -> Self {
        Self(StorageProviderInSyncPolicy(policy.0))
    }
}
