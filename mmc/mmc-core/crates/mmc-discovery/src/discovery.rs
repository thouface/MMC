//! mDNS/DNS-SD device discovery implementation

use async_trait::async_trait;
use futures::Stream;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

use crate::{DiscoveryEvent, Error, Result};

/// Device type enumeration
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

impl Default for DeviceType {
    fn default() -> Self {
        Self::Unknown
    }
}

impl From<&str> for DeviceType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "phone" => Self::Phone,
            "tablet" => Self::Tablet,
            "pc" | "desktop" => Self::Pc,
            "tv" => Self::Tv,
            "wearable" => Self::Wearable,
            _ => Self::Unknown,
        }
    }
}

/// Discovered device information
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

impl DeviceInfo {
    pub fn from_service_info(info: &ServiceInfo, txt_records: &HashMap<String, String>) -> Option<Self> {
        let id = info.get_id()?.to_string();
        let name = info.get_name().to_string();

        let ip = info.get addresses().first()?.to_string();
        let port = info.get_port();

        let device_type = txt_records.get("type").map(|s| DeviceType::from(s.as_str())).unwrap_or_default();
        let os_version = txt_records.get("os").cloned().unwrap_or_default();
        let app_version = txt_records.get("ver").cloned().unwrap_or_default();

        Some(DeviceInfo {
            id,
            name,
            device_type,
            os_version,
            app_version,
            ip,
            port,
            last_seen: chrono::Utc::now().timestamp(),
        })
    }
}

/// Internal discovered device with metadata
#[derive(Debug, Clone)]
pub struct Device {
    pub info: DeviceInfo,
    pub last_seen_instant: Instant,
    pub service_info: ServiceInfo,
}

impl Device {
    pub fn is_expired(&self, ttl_secs: u32) -> bool {
        self.last_seen_instant.elapsed() > Duration::from_secs(ttl_secs as u64 * 2)
    }
}

/// Discovery service events
#[derive(Debug, Clone)]
pub enum DiscoveryEvent {
    DeviceFound(DeviceInfo),
    DeviceUpdated(DeviceInfo),
    DeviceLost(String),
}

/// Service type for MMC
const SERVICE_TYPE: &str = "_mmc._tcp.local.";

/// Discovery service for mDNS-based device discovery
pub struct DiscoveryService {
    daemon: Option<ServiceDaemon>,
    browser: Option<mdns_sd::ServiceBrowser>,
    discovered: Arc<RwLock<HashMap<String, Device>>>,
    event_tx: broadcast::Sender<DiscoveryEvent>,
}

impl DiscoveryService {
    /// Create a new discovery service
    pub fn new() -> Result<Self> {
        let daemon = ServiceDaemon::new().map_err(|e| Error::Mdns(e.to_string()))?;
        let (event_tx, _) = broadcast::channel(100);

        Ok(Self {
            daemon: Some(daemon),
            browser: None,
            discovered: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        })
    }

    /// Start browsing for MMC devices
    pub fn start_browse(&mut self) -> Result<()> {
        let daemon = self.daemon.take().ok_or_else(|| Error::NotStarted)?;

        let browser = daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| Error::Mdns(e.to_string()))?;

        self.browser = Some(browser);

        // Keep daemon alive in background
        let daemon_handle = std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        });
        daemon_handle.detach();

        Ok(())
    }

    /// Register this device as an MMC service
    pub fn register_service(
        &self,
        device_id: &str,
        device_name: &str,
        device_type: DeviceType,
        os_version: &str,
        app_version: &str,
        port: u16,
    ) -> Result<()> {
        let daemon = ServiceDaemon::new().map_err(|e| Error::Mdns(e.to_string()))?;

        let props = HashMap::from([
            ("type".to_string(), format!("{:?}", device_type).to_lowercase()),
            ("os".to_string(), os_version.to_string()),
            ("ver".to_string(), app_version.to_string()),
        ]);

        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            device_id,
            device_name,
            Some(vec!["127.0.0.1".parse().unwrap()]), // Will be auto-detected
            port,
            props,
        )
        .map_err(|e| Error::Mdns(e.to_string()))?
        .enable_addr_auto();

        daemon
            .register(service_info)
            .map_err(|e| Error::Mdns(e.to_string()))?;

        Ok(())
    }

    /// Get a stream of discovery events
    pub fn events(&self) -> broadcast::Receiver<DiscoveryEvent> {
        self.event_tx.subscribe()
    }

    /// Get all currently discovered devices
    pub async fn get_discovered(&self) -> Vec<DeviceInfo> {
        let discovered = self.discovered.read().await;
        discovered.values().map(|d| d.info.clone()).collect()
    }

    /// Handle incoming service events
    pub async fn handle_event(&self, event: ServiceEvent) {
        match event {
            ServiceEvent::ServiceResolved(info) => {
                let txt_records: HashMap<String, String> = info
                    .get_properties()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                if let Some(device_info) = DeviceInfo::from_service_info(&info, &txt_records) {
                    debug!("Device discovered: {} ({})", device_info.name, device_info.id);

                    let mut discovered = self.discovered.write().await;
                    let is_new = !discovered.contains_key(&device_info.id);

                    let device = Device {
                        info: device_info.clone(),
                        last_seen_instant: Instant::now(),
                        service_info: info,
                    };

                    discovered.insert(device_info.id.clone(), device);

                    let evt = if is_new {
                        DiscoveryEvent::DeviceFound(device_info)
                    } else {
                        DiscoveryEvent::DeviceUpdated(device_info)
                    };

                    let _ = self.event_tx.send(evt);
                }
            }
            ServiceEvent::ServiceLost(service_name, _) => {
                let id = service_name.split('#').nth(1).unwrap_or(&service_name).to_string();
                debug!("Device lost: {}", id);

                let mut discovered = self.discovered.write().await;
                if discovered.remove(&id).is_some() {
                    let _ = self.event_tx.send(DiscoveryEvent::DeviceLost(id));
                }
            }
            _ => {}
        }
    }

    /// Clean up expired devices
    pub async fn cleanup_expired(&self, ttl_secs: u32) {
        let mut discovered = self.discovered.write().await;
        let expired: Vec<String> = discovered
            .iter()
            .filter(|(_, d)| d.is_expired(ttl_secs))
            .map(|(id, _)| id.clone())
            .collect();

        for id in expired {
            if let Some(device) = discovered.remove(&id) {
                debug!("Device expired: {}", device.info.name);
                let _ = self.event_tx.send(DiscoveryEvent::DeviceLost(id));
            }
        }
    }
}

impl Default for DiscoveryService {
    fn default() -> Self {
        Self::new().expect("Failed to create discovery service")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_type_from_str() {
        assert_eq!(DeviceType::from("phone"), DeviceType::Phone);
        assert_eq!(DeviceType::from("PC"), DeviceType::Pc);
        assert_eq!(DeviceType::from("unknown_type"), DeviceType::Unknown);
    }

    #[tokio::test]
    async fn test_discovery_service_creation() {
        let service = DiscoveryService::new();
        assert!(service.is_ok());
    }
}
