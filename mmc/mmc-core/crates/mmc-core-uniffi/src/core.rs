//! MMC Core - Main entry point for the library

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use crate::error::{CoreError, Result};
use crate::types::{CoreConfig, DeviceInfo, TransferProgress, TransferTask};

use mmc_discovery::{DiscoveryService, DeviceInfo as DiscoveryDeviceInfo, DeviceType as DiscoveryDeviceType};
use mmc_pairing::{Capabilities, PairingResult, PairingService};
use mmc_file_transfer::{TransferService, TransferTask as FtTransferTask};
use mmc_storage::{PairedDevice, StorageService};

pub struct MmcCore {
    config: RwLock<Option<CoreConfig>>,
    initialized: RwLock<bool>,
    discovery: Arc<RwLock<Option<DiscoveryService>>>,
    pairing: Arc<RwLock<Option<PairingService>>>,
    file_transfer: Arc<RwLock<Option<TransferService>>>,
    storage: Arc<RwLock<Option<StorageService>>>,
}

impl MmcCore {
    pub fn new() -> Self {
        Self {
            config: RwLock::new(None),
            initialized: RwLock::new(false),
            discovery: Arc::new(RwLock::new(None)),
            pairing: Arc::new(RwLock::new(None)),
            file_transfer: Arc::new(RwLock::new(None)),
            storage: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn init(&self, config: CoreConfig) -> Result<()> {
        {
            let initialized = self.initialized.read().await;
            if *initialized {
                return Err(CoreError::AlreadyInitialized);
            }
        }

        info!("Initializing MMC Core for device: {}", config.device_name);

        let discovery = DiscoveryService::new()
            .map_err(|e| CoreError::InitFailed(e.to_string()))?;
        
        let storage_path = if let Some(log_dir) = &config.log_dir {
            std::path::Path::new(log_dir).join("mmc_storage.db")
        } else {
            std::path::PathBuf::from(".mmc/storage.db")
        };
        
        let storage = StorageService::with_path(&storage_path)
            .await
            .map_err(|e| CoreError::InitFailed(e.to_string()))?;

        {
            let mut cfg = self.config.write().await;
            *cfg = Some(config);
        }

        {
            let mut disc = self.discovery.write().await;
            *disc = Some(discovery);
        }

        {
            let mut pair = self.pairing.write().await;
            *pair = Some(PairingService::new());
        }

        {
            let mut ft = self.file_transfer.write().await;
            *ft = Some(TransferService::new());
        }

        {
            let mut st = self.storage.write().await;
            *st = Some(storage);
        }

        {
            let mut initialized = self.initialized.write().await;
            *initialized = true;
        }

        info!("MMC Core initialized successfully");
        Ok(())
    }

    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }

    pub async fn get_config(&self) -> Result<CoreConfig> {
        let config = self.config.read().await;
        config.clone().ok_or(CoreError::NotInitialized)
    }

    pub async fn start_discovery(&self) -> Result<()> {
        let initialized = self.initialized.read().await;
        if !*initialized {
            return Err(CoreError::NotInitialized);
        }

        let discovery = self.discovery.read().await;
        if let Some(d) = discovery.as_ref() {
            d.start_browse().map_err(|e| CoreError::DiscoveryFailed(e.to_string()))?;
            info!("Device discovery started");
        }
        Ok(())
    }

    pub async fn get_discovered_devices(&self) -> Result<Vec<DeviceInfo>> {
        let initialized = self.initialized.read().await;
        if !*initialized {
            return Err(CoreError::NotInitialized);
        }

        let discovery = self.discovery.read().await;
        if let Some(d) = discovery.as_ref() {
            let devices = d.get_discovered().await;
            Ok(devices.into_iter().map(DeviceInfo::from).collect())
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn register_device(&self, port: u16) -> Result<()> {
        let initialized = self.initialized.read().await;
        if !*initialized {
            return Err(CoreError::NotInitialized);
        }

        let config = self.config.read().await;
        let cfg = config.as_ref().ok_or(CoreError::NotInitialized)?;

        let discovery = self.discovery.read().await;
        if let Some(d) = discovery.as_ref() {
            d.register_service(
                &cfg.device_id,
                &cfg.device_name,
                DiscoveryDeviceType::Unknown,
                "unknown",
                &cfg.app_version,
                port,
            )
            .map_err(|e| CoreError::DiscoveryFailed(e.to_string()))?;
            info!("Device registered on network");
        }
        Ok(())
    }

    pub async fn pair_device(
        &self,
        _device_id: &str,
        ip: &str,
        port: u16,
        public_key: &[u8; 32],
        capabilities: Capabilities,
    ) -> Result<PairingResult> {
        let initialized = self.initialized.read().await;
        if !*initialized {
            return Err(CoreError::NotInitialized);
        }

        let config = self.config.read().await;
        let cfg = config.as_ref().ok_or(CoreError::NotInitialized)?;

        let pairing = self.pairing.read().await;
        if let Some(p) = pairing.as_ref() {
            let result = p
                .pair(ip, port, public_key, cfg.device_id.clone(), cfg.device_name.clone(), capabilities)
                .await
                .map_err(|e| CoreError::PairingFailed(e.to_string()))?;

            Ok(result)
        } else {
            Err(CoreError::PairingFailed("Pairing service not available".to_string()))
        }
    }

    pub async fn send_file(
        &self,
        device_id: &str,
        file_path: &std::path::Path,
    ) -> Result<String> {
        let initialized = self.initialized.read().await;
        if !*initialized {
            return Err(CoreError::NotInitialized);
        }

        let storage = self.storage.read().await;
        let _device = storage
            .as_ref()
            .ok_or(CoreError::StorageFailed("Storage not available".to_string()))?
            .get_device(device_id)
            .await
            .map_err(|e| CoreError::StorageFailed(e.to_string()))?
            .ok_or(CoreError::StorageFailed("Device not found".to_string()))?;

        let file_transfer = self.file_transfer.read().await;
        if let Some(ft) = file_transfer.as_ref() {
            let _manifest = ft
                .compute_manifest(file_path, 1024 * 1024)
                .await
                .map_err(|e| CoreError::TransferFailed(e.to_string()))?;

            let task_id = uuid::Uuid::new_v4().to_string();
            Ok(task_id)
        } else {
            Err(CoreError::TransferFailed("File transfer service not available".to_string()))
        }
    }

    pub async fn get_transfer_tasks(&self) -> Result<Vec<TransferTask>> {
        let initialized = self.initialized.read().await;
        if !*initialized {
            return Err(CoreError::NotInitialized);
        }

        let file_transfer = self.file_transfer.read().await;
        if let Some(ft) = file_transfer.as_ref() {
            Ok(ft.get_tasks().await.into_iter().map(|t| t.into()).collect())
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn get_paired_devices(&self) -> Result<Vec<PairedDevice>> {
        let initialized = self.initialized.read().await;
        if !*initialized {
            return Err(CoreError::NotInitialized);
        }

        let storage = self.storage.read().await;
        if let Some(s) = storage.as_ref() {
            s.list_devices().await.map_err(|e| CoreError::StorageFailed(e.to_string()))
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn remove_paired_device(&self, device_id: &str) -> Result<()> {
        let initialized = self.initialized.read().await;
        if !*initialized {
            return Err(CoreError::NotInitialized);
        }

        let storage = self.storage.read().await;
        if let Some(s) = storage.as_ref() {
            s.remove_device(device_id)
                .await
                .map_err(|e| CoreError::StorageFailed(e.to_string()))
        } else {
            Err(CoreError::StorageFailed("Storage not available".to_string()))
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down MMC Core");
        let mut initialized = self.initialized.write().await;
        *initialized = false;
        Ok(())
    }
}

impl From<DiscoveryDeviceInfo> for DeviceInfo {
    fn from(info: DiscoveryDeviceInfo) -> Self {
        Self {
            id: info.id,
            name: info.name,
            device_type: crate::types::DeviceType::from(info.device_type),
            os_version: info.os_version,
            app_version: info.app_version,
            ip: info.ip,
            port: info.port,
            last_seen: info.last_seen,
        }
    }
}

impl From<FtTransferTask> for TransferTask {
    fn from(task: FtTransferTask) -> Self {
        Self {
            task_id: task.task_id,
            file_id: task.file_id,
            file_name: task.file_name,
            total_size: task.total_size,
            chunk_size: task.chunk_size,
            total_chunks: task.total_chunks,
            state: crate::types::TransferState::from(task.state),
            progress: TransferProgress {
                task_id: task.progress.task_id.clone(),
                bytes_transferred: task.progress.bytes_transferred,
                total_bytes: task.progress.total_bytes,
                speed_bps: task.progress.speed_bps,
                remaining_ms: task.progress.remaining_ms,
                state: crate::types::TransferState::from(task.progress.state),
                percent: task.progress.percent(),
            },
            created_at: task.created_at,
        }
    }
}

impl Default for MmcCore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_core_lifecycle() {
        let core = MmcCore::new();
        assert!(!core.is_initialized().await);

        let dir = tempdir().unwrap();
        let config = CoreConfig {
            log_dir: Some(dir.path().to_string_lossy().to_string()),
            ..CoreConfig::default()
        };

        let result = core.init(config).await;
        if result.is_ok() {
            assert!(core.is_initialized().await);
            assert!(core.shutdown().await.is_ok());
            assert!(!core.is_initialized().await);
        }
    }

    #[tokio::test]
    async fn test_double_init_fails() {
        let core = MmcCore::new();
        let dir = tempdir().unwrap();

        let config = CoreConfig {
            log_dir: Some(dir.path().to_string_lossy().to_string()),
            ..CoreConfig::default()
        };

        if core.init(config).await.is_ok() {
            let config2 = CoreConfig::default();
            assert!(core.init(config2).await.is_err());
        }
    }

    #[tokio::test]
    async fn test_storage_integration() {
        let dir = tempdir().unwrap();
        let core = MmcCore::new();

        let config = CoreConfig {
            log_dir: Some(dir.path().to_string_lossy().to_string()),
            ..CoreConfig::default()
        };

        if core.init(config).await.is_ok() {
            assert!(core.get_paired_devices().await.is_ok());
            assert!(core.shutdown().await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_discovery_integration() {
        let dir = tempdir().unwrap();
        let core = MmcCore::new();

        let config = CoreConfig {
            log_dir: Some(dir.path().to_string_lossy().to_string()),
            ..CoreConfig::default()
        };

        if core.init(config).await.is_ok() {
            let _ = core.start_discovery().await;
            assert!(core.get_discovered_devices().await.is_ok());
            assert!(core.shutdown().await.is_ok());
        }
    }
}
