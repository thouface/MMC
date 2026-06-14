//! Device discovery module using mDNS/DNS-SD
//! Discovers MMC devices on the local network

pub mod error;

/// Service type for MMC
const SERVICE_TYPE: &str = "_mmc._tcp.local.";

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

/// Discovered device information
#[derive(Debug, Clone)]
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

/// Discovery service events
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    DeviceFound(DeviceInfo),
    DeviceUpdated(DeviceInfo),
    DeviceLost(String),
}

/// Discovery service placeholder
/// Note: Full mDNS implementation requires platform-specific setup
pub struct DiscoveryService;

impl DiscoveryService {
    pub fn new() -> Self {
        Self
    }

    /// Get service type
    pub fn service_type() -> &'static str {
        SERVICE_TYPE
    }
}

impl Default for DiscoveryService {
    fn default() -> Self {
        Self::new()
    }
}

pub use error::{Error, Result};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_type() {
        assert_eq!(DiscoveryService::service_type(), "_mmc._tcp.local.");
    }

    #[tokio::test]
    async fn test_discovery_service_creation() {
        let service = DiscoveryService::new();
        assert_eq!(DiscoveryService::service_type(), "_mmc._tcp.local.");
    }
}
