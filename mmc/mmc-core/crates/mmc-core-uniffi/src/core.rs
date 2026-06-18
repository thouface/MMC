//! MMC Core - Main entry point for the library
//!
//! This module implements the unified API for all MMC functionality.
//! The API is synchronous for easier FFI integration.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};

use crate::error::CoreError;
use crate::types::{CoreConfig, DeviceInfo, TransferProgress, TransferTask};

use mmc_discovery::{DiscoveryService, DeviceType as DiscoveryDeviceType, DeviceInfo as DiscoveryDeviceInfo};
use mmc_pairing::{PairingService, Capabilities, PairingResult as InnerPairingResult};
use mmc_file_transfer::{TransferService, TransferTask as FtTransferTask};
use mmc_storage::{StorageService, PairedDevice};
use mmc_media_service::{MirroringSession, SessionConfig as InnerSessionConfig};
use mmc_protocol::{PixelFormat, SampleFormat};

/// Core status for FFI (mirrors CoreError but is a simple enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CoreStatus {
    Ok = 0,
    NotInitialized = 1,
    AlreadyInitialized = 2,
    InitFailed = 3,
    Io = 4,
    Network = 5,
    Protocol = 6,
    Security = 7,
    StorageFailed = 8,
    TransferFailed = 9,
    DiscoveryFailed = 10,
    PairingFailed = 11,
    InvalidState = 12,
    Timeout = 13,
    Cancelled = 14,
    Unknown = 99,
}

impl From<&CoreError> for CoreStatus {
    fn from(err: &CoreError) -> Self {
        match err {
            CoreError::NotInitialized => Self::NotInitialized,
            CoreError::AlreadyInitialized => Self::AlreadyInitialized,
            CoreError::InitFailed(_) => Self::InitFailed,
            CoreError::Io(_) => Self::Io,
            CoreError::Network(_) => Self::Network,
            CoreError::Protocol(_) => Self::Protocol,
            CoreError::Security(_) => Self::Security,
            CoreError::StorageFailed(_) => Self::StorageFailed,
            CoreError::TransferFailed(_) => Self::TransferFailed,
            CoreError::DiscoveryFailed(_) => Self::DiscoveryFailed,
            CoreError::PairingFailed(_) => Self::PairingFailed,
            CoreError::InvalidState(_) => Self::InvalidState,
            CoreError::Timeout => Self::Timeout,
            CoreError::Cancelled => Self::Cancelled,
            CoreError::Unknown(_) => Self::Unknown,
        }
    }
}

/// Pairing request from FFI
#[derive(Debug, Clone)]
pub struct PairingRequest {
    pub device_id: String,
    pub device_name: String,
    pub ip: String,
    pub port: u16,
    pub file_transfer: bool,
    pub screen_mirror: bool,
    pub remote_control: bool,
    pub clipboard_sync: bool,
}

/// Pairing result for FFI
#[derive(Debug, Clone)]
pub struct PairingResult {
    pub success: bool,
    pub pairing_id: String,
    pub error_message: Option<String>,
}

impl From<InnerPairingResult> for PairingResult {
    fn from(result: InnerPairingResult) -> Self {
        match result {
            InnerPairingResult::Success { device_id, .. } => Self {
                success: true,
                pairing_id: device_id,
                error_message: None,
            },
            InnerPairingResult::Rejected { pairing_id, reason } => Self {
                success: false,
                pairing_id,
                error_message: Some(reason),
            },
            InnerPairingResult::Error { pairing_id, error } => Self {
                success: false,
                pairing_id,
                error_message: Some(error),
            },
        }
    }
}

/// Session configuration from FFI
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub video_width: u32,
    pub video_height: u32,
    pub video_format: String,
    pub frame_rate: u32,
    pub audio_sample_rate: u32,
    pub audio_channels: u32,
    pub audio_format: String,
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub state: String,
    pub video_frames: u64,
    pub audio_frames: u64,
    pub input_events: u64,
    pub duration: Option<f32>,
}

impl From<mmc_media_service::SessionStats> for SessionStats {
    fn from(stats: mmc_media_service::SessionStats) -> Self {
        Self {
            state: format!("{:?}", stats.state),
            video_frames: stats.video_frames,
            audio_frames: stats.audio_frames,
            input_events: stats.input_events,
            duration: stats.duration,
        }
    }
}

/// MMC Core implementation
pub struct MmcCore {
    config: RwLock<Option<CoreConfig>>,
    initialized: RwLock<bool>,
    discovery: Arc<RwLock<Option<DiscoveryService>>>,
    pairing: Arc<RwLock<Option<PairingService>>>,
    file_transfer: Arc<RwLock<Option<TransferService>>>,
    storage: Arc<RwLock<Option<StorageService>>>,
    mirror_session: Arc<RwLock<Option<MirroringSession>>>,
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
            mirror_session: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the core library (synchronous wrapper)
    pub fn init(&self, config: CoreConfig) -> CoreStatus {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let initialized = runtime.block_on(async {
            *self.initialized.read().await
        });

        if initialized {
            return CoreStatus::AlreadyInitialized;
        }

        info!("Initializing MMC Core for device: {}", config.device_name);

        let discovery = match DiscoveryService::new() {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to create discovery service: {}", e);
                return CoreStatus::InitFailed;
            }
        };

        let storage_path = if let Some(ref log_dir) = config.log_dir {
            std::path::Path::new(log_dir).join("mmc_storage.db")
        } else {
            std::path::PathBuf::from(".mmc/storage.db")
        };

        let storage = runtime.block_on(async {
            StorageService::with_path(&storage_path).await
        });

        let storage = match storage {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to create storage service: {}", e);
                return CoreStatus::InitFailed;
            }
        };

        // Initialize all services
        runtime.block_on(async {
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
        });

        info!("MMC Core initialized successfully");
        CoreStatus::Ok
    }

    pub fn is_initialized(&self) -> bool {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            *self.initialized.read().await
        })
    }

    pub fn get_config(&self) -> Option<CoreConfig> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            self.config.read().await.clone()
        })
    }

    pub fn shutdown(&self) -> CoreStatus {
        info!("Shutting down MMC Core");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            {
                let mut session = self.mirror_session.write().await;
                if let Some(ref mut s) = *session {
                    let _ = s.stop();
                }
                *session = None;
            }
            let mut initialized = self.initialized.write().await;
            *initialized = false;
        });
        CoreStatus::Ok
    }

    // Discovery methods
    pub fn start_discovery(&self) -> CoreStatus {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let initialized = *self.initialized.read().await;
            if !initialized {
                return CoreStatus::NotInitialized;
            }

            let discovery = self.discovery.read().await;
            if let Some(d) = discovery.as_ref() {
                if let Err(e) = d.start_browse() {
                    error!("Failed to start discovery: {}", e);
                    return CoreStatus::DiscoveryFailed;
                }
                info!("Device discovery started");
            }
            CoreStatus::Ok
        })
    }

    pub fn get_discovered_devices(&self) -> Vec<DeviceInfo> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let initialized = *self.initialized.read().await;
            if !initialized {
                return Vec::new();
            }

            let discovery = self.discovery.read().await;
            if let Some(d) = discovery.as_ref() {
                let devices = d.get_discovered().await;
                devices.into_iter().map(DeviceInfo::from).collect()
            } else {
                Vec::new()
            }
        })
    }

    pub fn register_device(&self, port: u16) -> CoreStatus {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let initialized = *self.initialized.read().await;
            if !initialized {
                return CoreStatus::NotInitialized;
            }

            let config = self.config.read().await;
            let cfg = match config.as_ref() {
                Some(c) => c,
                None => return CoreStatus::NotInitialized,
            };

            let discovery = self.discovery.read().await;
            if let Some(d) = discovery.as_ref() {
                if let Err(e) = d.register_service(
                    &cfg.device_id,
                    &cfg.device_name,
                    DiscoveryDeviceType::Unknown,
                    "unknown",
                    &cfg.app_version,
                    port,
                ) {
                    error!("Failed to register device: {}", e);
                    return CoreStatus::DiscoveryFailed;
                }
                info!("Device registered on network");
            }
            CoreStatus::Ok
        })
    }

    // Pairing methods
    pub fn pair_device(&self, request: PairingRequest) -> Option<PairingResult> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let initialized = *self.initialized.read().await;
            if !initialized {
                return None;
            }

            let config = self.config.read().await;
            let cfg = match config.as_ref() {
                Some(c) => c,
                None => return None,
            };

            let pairing = self.pairing.read().await;
            if let Some(p) = pairing.as_ref() {
                let capabilities = Capabilities {
                    file_transfer: request.file_transfer,
                    screen_mirror: request.screen_mirror,
                    remote_control: request.remote_control,
                    clipboard_sync: request.clipboard_sync,
                };

                let mut keypair = [0u8; 32];
                use rand::RngCore;
                rand::thread_rng().fill_bytes(&mut keypair);

                match p.pair(
                    &request.ip,
                    request.port,
                    &keypair,
                    cfg.device_id.clone(),
                    cfg.device_name.clone(),
                    capabilities,
                ).await {
                    Ok(result) => Some(PairingResult::from(result)),
                    Err(_) => None,
                }
            } else {
                None
            }
        })
    }

    // File transfer methods
    pub fn send_file(&self, _device_id: &str, file_path: &str) -> Option<String> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let initialized = *self.initialized.read().await;
            if !initialized {
                return None;
            }

            let path = std::path::Path::new(file_path);
            if !path.exists() {
                return None;
            }

            let file_transfer = self.file_transfer.read().await;
            if let Some(ft) = file_transfer.as_ref() {
                match ft.compute_manifest(path, 1024 * 1024).await {
                    Ok(_manifest) => {
                        let task_id = uuid::Uuid::new_v4().to_string();
                        Some(task_id)
                    }
                    Err(_) => None,
                }
            } else {
                None
            }
        })
    }

    pub fn get_transfer_tasks(&self) -> Vec<TransferTask> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let initialized = *self.initialized.read().await;
            if !initialized {
                return Vec::new();
            }

            let file_transfer = self.file_transfer.read().await;
            if let Some(ft) = file_transfer.as_ref() {
                let tasks = ft.get_tasks().await;
                tasks.into_iter().map(TransferTask::from).collect()
            } else {
                Vec::new()
            }
        })
    }

    pub fn cancel_transfer(&self, _task_id: &str) -> CoreStatus {
        // TODO: Implement actual cancellation
        CoreStatus::Ok
    }

    // Paired devices methods
    pub fn get_paired_devices(&self) -> Vec<DeviceInfo> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let initialized = *self.initialized.read().await;
            if !initialized {
                return Vec::new();
            }

            let storage = self.storage.read().await;
            if let Some(s) = storage.as_ref() {
                match s.list_devices().await {
                    Ok(devices) => devices.into_iter().map(|d| DeviceInfo::from_paired_device(&d)).collect(),
                    Err(_) => Vec::new(),
                }
            } else {
                Vec::new()
            }
        })
    }

    pub fn remove_paired_device(&self, device_id: &str) -> CoreStatus {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let initialized = *self.initialized.read().await;
            if !initialized {
                return CoreStatus::NotInitialized;
            }

            let storage = self.storage.read().await;
            if let Some(s) = storage.as_ref() {
                match s.remove_device(device_id).await {
                    Ok(_) => CoreStatus::Ok,
                    Err(_) => CoreStatus::StorageFailed,
                }
            } else {
                CoreStatus::StorageFailed
            }
        })
    }

    // Mirror session methods
    pub fn start_mirror_session(&self, config: SessionConfig) -> CoreStatus {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let initialized = *self.initialized.read().await;
            if !initialized {
                return CoreStatus::NotInitialized;
            }

            let video_format = match config.video_format.as_str() {
                "RGBA8888" => PixelFormat::Rgba8888,
                "BGRA8888" => PixelFormat::Bgra8888,
                "RGB565" => PixelFormat::Rgb565,
                "YUV420P" => PixelFormat::Yuv420p,
                "NV12" => PixelFormat::Nv12,
                _ => PixelFormat::Rgba8888,
            };

            let audio_format = match config.audio_format.as_str() {
                "U8" => SampleFormat::U8,
                "S16" => SampleFormat::S16,
                "S32" => SampleFormat::S32,
                "F32" => SampleFormat::F32,
                _ => SampleFormat::S16,
            };

            let inner_config = InnerSessionConfig {
                video_width: config.video_width,
                video_height: config.video_height,
                video_format,
                frame_rate: config.frame_rate,
                audio_sample_rate: config.audio_sample_rate,
                audio_channels: config.audio_channels,
                audio_format,
            };

            let mut session = MirroringSession::new();
            if let Err(e) = session.configure(inner_config) {
                error!("Failed to configure mirror session: {:?}", e);
                return CoreStatus::InitFailed;
            }

            if let Err(e) = session.start() {
                error!("Failed to start mirror session: {:?}", e);
                return CoreStatus::InvalidState;
            }

            let mut mirror = self.mirror_session.write().await;
            *mirror = Some(session);

            info!("Mirror session started");
            CoreStatus::Ok
        })
    }

    pub fn stop_mirror_session(&self) -> CoreStatus {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let mut session = self.mirror_session.write().await;
            if let Some(ref mut s) = *session {
                if let Err(e) = s.stop() {
                    error!("Failed to stop mirror session: {:?}", e);
                    return CoreStatus::InvalidState;
                }
            }
            *session = None;
            info!("Mirror session stopped");
            CoreStatus::Ok
        })
    }

    pub fn get_session_stats(&self) -> Option<SessionStats> {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let session = self.mirror_session.read().await;
            session.as_ref().map(|s| SessionStats::from(s.get_stats()))
        })
    }
}

impl Default for MmcCore {
    fn default() -> Self {
        Self::new()
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

impl DeviceInfo {
    /// Create DeviceInfo from PairedDevice
    pub fn from_paired_device(device: &PairedDevice) -> Self {
        Self {
            id: device.device_id.clone(),
            name: device.device_name.clone(),
            device_type: crate::types::DeviceType::from(device.device_type),
            os_version: device.os_version.clone(),
            app_version: device.app_version.clone(),
            ip: device.ip_address.clone(),
            port: device.port,
            last_seen: device.last_connected_at.unwrap_or(0),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_core_lifecycle() {
        let core = MmcCore::new();
        assert!(!core.is_initialized());

        let dir = tempdir().unwrap();
        let config = CoreConfig {
            device_id: "test-device".to_string(),
            device_name: "Test Device".to_string(),
            device_type: crate::types::DeviceType::Pc,
            app_version: "1.0.0".to_string(),
            log_dir: Some(dir.path().to_string_lossy().to_string()),
        };

        let status = core.init(config);
        assert_eq!(status, CoreStatus::Ok);
        assert!(core.is_initialized());

        let status = core.shutdown();
        assert_eq!(status, CoreStatus::Ok);
        assert!(!core.is_initialized());
    }

    #[test]
    fn test_double_init() {
        let core = MmcCore::new();
        let dir = tempdir().unwrap();

        let config = CoreConfig {
            device_id: "test-device".to_string(),
            device_name: "Test Device".to_string(),
            device_type: crate::types::DeviceType::Pc,
            app_version: "1.0.0".to_string(),
            log_dir: Some(dir.path().to_string_lossy().to_string()),
        };

        let status1 = core.init(config.clone());
        assert_eq!(status1, CoreStatus::Ok);

        let status2 = core.init(config);
        assert_eq!(status2, CoreStatus::AlreadyInitialized);

        let _ = core.shutdown();
    }

    #[test]
    fn test_core_status_conversion() {
        let err = CoreError::NotInitialized;
        assert_eq!(CoreStatus::from(&err), CoreStatus::NotInitialized);

        let err = CoreError::DiscoveryFailed("test".to_string());
        assert_eq!(CoreStatus::from(&err), CoreStatus::DiscoveryFailed);
    }

    #[test]
    fn test_pairing_result_from_inner() {
        let inner = InnerPairingResult::Success {
            device_id: "device-123".to_string(),
            device_name: "Test Device".to_string(),
            shared_secret: [0u8; 32],
        };
        let result = PairingResult::from(inner);
        assert!(result.success);
        assert_eq!(result.pairing_id, "device-123");
        assert!(result.error_message.is_none());

        let inner = InnerPairingResult::Rejected {
            pairing_id: "pair-123".to_string(),
            reason: "User rejected".to_string(),
        };
        let result = PairingResult::from(inner);
        assert!(!result.success);
        assert_eq!(result.error_message, Some("User rejected".to_string()));
    }
}
