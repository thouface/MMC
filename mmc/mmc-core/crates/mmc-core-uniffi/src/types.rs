//! Core types used across MMC

use mmc_discovery::DeviceType as DiscoveryDeviceType;
use mmc_file_transfer::TransferState as FtTransferState;
use mmc_storage::DeviceType as StorageDeviceType;
use serde::{Deserialize, Serialize};

/// Device type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    Unknown,
    Phone,
    Tablet,
    Pc,
    Tv,
    Wearable,
}

impl From<DiscoveryDeviceType> for DeviceType {
    fn from(dt: DiscoveryDeviceType) -> Self {
        match dt {
            DiscoveryDeviceType::Unknown => Self::Unknown,
            DiscoveryDeviceType::Phone => Self::Phone,
            DiscoveryDeviceType::Tablet => Self::Tablet,
            DiscoveryDeviceType::Pc => Self::Pc,
            DiscoveryDeviceType::Tv => Self::Tv,
            DiscoveryDeviceType::Wearable => Self::Wearable,
        }
    }
}

impl From<StorageDeviceType> for DeviceType {
    fn from(dt: StorageDeviceType) -> Self {
        match dt {
            StorageDeviceType::Unknown => Self::Unknown,
            StorageDeviceType::Phone => Self::Phone,
            StorageDeviceType::Tablet => Self::Tablet,
            StorageDeviceType::Pc => Self::Pc,
            StorageDeviceType::Tv => Self::Tv,
            StorageDeviceType::Wearable => Self::Wearable,
        }
    }
}

/// Device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub os_version: String,
    pub app_version: String,
    pub ip: String,
    pub port: u16,
    pub last_seen: i64,
}

/// Transfer state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferState {
    Idle,
    Preparing,
    Transferring,
    Paused,
    Completed,
    Failed,
    Canceled,
}

impl From<FtTransferState> for TransferState {
    fn from(state: FtTransferState) -> Self {
        match state {
            FtTransferState::Idle => Self::Idle,
            FtTransferState::Preparing => Self::Preparing,
            FtTransferState::Transferring => Self::Transferring,
            FtTransferState::Paused => Self::Paused,
            FtTransferState::Completed => Self::Completed,
            FtTransferState::Failed => Self::Failed,
            FtTransferState::Canceled => Self::Canceled,
        }
    }
}

/// Transfer progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub task_id: String,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub speed_bps: u64,
    pub remaining_ms: u64,
    pub state: TransferState,
    pub percent: f32,
}

/// Transfer task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferTask {
    pub task_id: String,
    pub file_id: String,
    pub file_name: String,
    pub total_size: u64,
    pub chunk_size: u32,
    pub total_chunks: u32,
    pub state: TransferState,
    pub progress: TransferProgress,
    pub created_at: i64,
}

/// Core configuration
#[derive(Debug, Clone)]
pub struct CoreConfig {
    pub device_id: String,
    pub device_name: String,
    pub device_type: DeviceType,
    pub app_version: String,
    pub log_dir: Option<String>,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            device_id: uuid::Uuid::new_v4().to_string(),
            device_name: hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "Unknown Device".to_string()),
            device_type: DeviceType::Phone,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            log_dir: None,
        }
    }
}
