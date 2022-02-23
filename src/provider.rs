use windows::{
    core,
    Win32::Storage::CloudFilters::{
        self, CfDisconnectSyncRoot, CfQuerySyncProviderStatus, CfReportSyncStatus,
        CfUpdateSyncProviderStatus, CF_CONNECTION_KEY, CF_SYNC_PROVIDER_STATUS, CF_SYNC_STATUS,
    },
};

use crate::{command::SyncStatus, logger::Reason};

#[derive(Debug, Clone)]
pub struct Provider {
    connection_key: isize,
}

// TODO: this should disconnect on drop ONLY if it's the main provider that was returned by connect
// in this case, it should also drop the weak arc stored in the CallbackContext
impl Provider {
    pub fn new(connection_key: isize) -> Self {
        Self { connection_key }
    }

    pub fn state(&self) -> core::Result<ProviderStatus> {
        unsafe {
            CfQuerySyncProviderStatus(CF_CONNECTION_KEY(self.connection_key))
                .map(|status| status.into())
        }
    }

    pub fn set_state(&self, status: ProviderStatus) -> core::Result<()> {
        unsafe { CfUpdateSyncProviderStatus(CF_CONNECTION_KEY(self.connection_key), status.into()) }
    }

    // TODO: Pass the path of the sync root here, does this function work if the
    // path leads to a descendant of the sync root?
    // rename this to something like `fail` or `set_error` or `set_reason`
    // https://docs.microsoft.com/en-us/windows/win32/api/cfapi/ns-cfapi-cf_placeholder_standard_info
    // I could get the sync root path from the SyncRootFileId ^
    pub fn set_default_availability(&self, status: Reason) -> core::Result<()> {
        unsafe {
            CfReportSyncStatus(
                "TODO",
                &SyncStatus::from(status) as *const _ as *const CF_SYNC_STATUS,
            )
        }
    }

    // TODO: Same as above without the second param
    // pub fn clear_status() -> {}

    pub fn disconnect(&self) -> core::Result<()> {
        unsafe { CfDisconnectSyncRoot(&CF_CONNECTION_KEY(self.connection_key)) }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ProviderStatus {
    Disconnected,
    Idle,
    PopulateNamespace,
    PopulateMetadata,
    PopulateContent,
    SyncIncremental,
    SyncFull,
    ConnectivityLost,
    ClearFlags,
    Terminated,
    Error,
}

impl From<ProviderStatus> for CF_SYNC_PROVIDER_STATUS {
    fn from(status: ProviderStatus) -> Self {
        match status {
            ProviderStatus::Disconnected => CloudFilters::CF_PROVIDER_STATUS_DISCONNECTED,
            ProviderStatus::Idle => CloudFilters::CF_PROVIDER_STATUS_IDLE,
            ProviderStatus::PopulateNamespace => {
                CloudFilters::CF_PROVIDER_STATUS_POPULATE_NAMESPACE
            }
            ProviderStatus::PopulateMetadata => CloudFilters::CF_PROVIDER_STATUS_POPULATE_METADATA,
            ProviderStatus::PopulateContent => CloudFilters::CF_PROVIDER_STATUS_POPULATE_CONTENT,
            ProviderStatus::SyncIncremental => CloudFilters::CF_PROVIDER_STATUS_SYNC_INCREMENTAL,
            ProviderStatus::SyncFull => CloudFilters::CF_PROVIDER_STATUS_SYNC_FULL,
            ProviderStatus::ConnectivityLost => CloudFilters::CF_PROVIDER_STATUS_CONNECTIVITY_LOST,
            ProviderStatus::ClearFlags => CloudFilters::CF_PROVIDER_STATUS_CLEAR_FLAGS,
            ProviderStatus::Terminated => CloudFilters::CF_PROVIDER_STATUS_TERMINATED,
            ProviderStatus::Error => CloudFilters::CF_PROVIDER_STATUS_ERROR,
        }
    }
}

impl From<CF_SYNC_PROVIDER_STATUS> for ProviderStatus {
    fn from(status: CF_SYNC_PROVIDER_STATUS) -> Self {
        match status {
            CloudFilters::CF_PROVIDER_STATUS_DISCONNECTED => Self::Disconnected,
            CloudFilters::CF_PROVIDER_STATUS_IDLE => Self::Idle,
            CloudFilters::CF_PROVIDER_STATUS_POPULATE_NAMESPACE => Self::PopulateNamespace,
            CloudFilters::CF_PROVIDER_STATUS_POPULATE_METADATA => Self::PopulateContent,
            CloudFilters::CF_PROVIDER_STATUS_POPULATE_CONTENT => Self::PopulateContent,
            CloudFilters::CF_PROVIDER_STATUS_SYNC_INCREMENTAL => Self::SyncIncremental,
            CloudFilters::CF_PROVIDER_STATUS_SYNC_FULL => Self::SyncFull,
            CloudFilters::CF_PROVIDER_STATUS_CONNECTIVITY_LOST => Self::ConnectivityLost,
            CloudFilters::CF_PROVIDER_STATUS_CLEAR_FLAGS => Self::ClearFlags,
            CloudFilters::CF_PROVIDER_STATUS_TERMINATED => Self::Terminated,
            CloudFilters::CF_PROVIDER_STATUS_ERROR => Self::Error,
            _ => unreachable!(),
        }
    }
}
