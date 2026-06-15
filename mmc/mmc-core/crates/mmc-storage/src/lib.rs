//! Storage module for persistent data
//! SQLite-based storage for pairing records and configuration

pub mod error;

pub use error::{Error, Result};

/// Device type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    Unknown,
    Phone,
    Tablet,
    Pc,
    Tv,
    Wearable,
}

impl Default for DeviceType {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Paired device record
#[derive(Debug, Clone)]
pub struct PairedDevice {
    pub device_id: String,
    pub device_name: String,
    pub device_type: DeviceType,
    pub os_version: String,
    pub app_version: String,
    pub ip_address: String,
    pub port: u16,
    pub public_key_fingerprint: String,
    pub paired_at: i64,
    pub last_connected_at: Option<i64>,
    pub trust_level: i32,
}

/// Storage service placeholder
pub struct StorageService;

impl StorageService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StorageService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_storage_service_creation() {
        let service = StorageService::new();
        // Placeholder test
        assert!(true);
    }
}
