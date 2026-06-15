//! Device discovery module using mDNS/DNS-SD
//! Discovers MMC devices on the local network

pub mod discovery;
pub mod error;

pub use discovery::{
    DeviceInfo, DeviceType, DiscoveryEvent, DiscoveryService,
};
pub use error::{Error, Result};

/// Service type for MMC
pub const SERVICE_TYPE: &str = "_mmc._tcp.local.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_type() {
        assert_eq!(DiscoveryService::service_type(), "_mmc._tcp.local.");
        assert_eq!(SERVICE_TYPE, "_mmc._tcp.local.");
    }

    #[tokio::test]
    async fn test_discovery_service_creation() {
        let service = DiscoveryService::new();
        assert!(service.is_ok());
    }
}
