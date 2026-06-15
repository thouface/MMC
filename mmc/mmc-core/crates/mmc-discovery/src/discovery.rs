//! mDNS/DNS-SD device discovery implementation

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info};

use crate::{Error, Result};

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
    /// Create DeviceInfo from service name, host, port and txt records
    pub fn new(
        service_name: &str,
        host: &str,
        port: u16,
        txt_records: &HashMap<String, String>,
    ) -> Self {
        let parts: Vec<&str> = service_name.split("._mmc._tcp.local.").collect();
        let name = parts.first().unwrap_or(&service_name).to_string();

        let device_type = txt_records
            .get("type")
            .map(|s| DeviceType::from(s.as_str()))
            .unwrap_or_default();
        let os_version = txt_records.get("os").cloned().unwrap_or_default();
        let app_version = txt_records.get("ver").cloned().unwrap_or_default();

        Self {
            id: service_name.to_string(),
            name,
            device_type,
            os_version,
            app_version,
            ip: host.to_string(),
            port,
            last_seen: chrono::Utc::now().timestamp(),
        }
    }
}

/// Internal discovered device with metadata
#[derive(Debug, Clone)]
pub struct Device {
    pub info: DeviceInfo,
    pub last_seen_instant: Instant,
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
            discovered: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
        })
    }

    /// Get service type
    pub fn service_type() -> &'static str {
        SERVICE_TYPE
    }

    /// Start browsing for MMC devices and handle events
    pub fn start_browse(&self) -> Result<()> {
        let daemon = self.daemon.as_ref().ok_or_else(|| Error::NotStarted)?;

        let discovered = self.discovered.clone();
        let event_tx = self.event_tx.clone();

        // Create a browser
        let browser = daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| Error::Mdns(e.to_string()))?;

        // Spawn async task to handle events
        tokio::spawn(async move {
            let receiver = browser;
            while let Ok(event) = receiver.recv_async().await {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        let service_name = info.get_fullname().to_string();
                        let host = info.get_hostname();
                        let port = info.get_port();

                        // Build txt records from properties
                        let txt_records: HashMap<String, String> = info
                            .get_properties()
                            .iter()
                            .filter_map(|prop| {
                                let key = prop.key().to_string();
                                let value = prop.val().and_then(|v| {
                                    String::from_utf8(v.to_vec()).ok()
                                }).unwrap_or_default();
                                Some((key, value))
                            })
                            .collect();

                        let device_info =
                            DeviceInfo::new(&service_name, host, port, &txt_records);

                        debug!(
                            "Device discovered: {} ({})",
                            device_info.name, device_info.id
                        );

                        let mut discovered = discovered.write().await;
                        let is_new = !discovered.contains_key(&device_info.id);

                        let device = Device {
                            info: device_info.clone(),
                            last_seen_instant: Instant::now(),
                        };

                        discovered.insert(device_info.id.clone(), device);

                        let evt = if is_new {
                            DiscoveryEvent::DeviceFound(device_info)
                        } else {
                            DiscoveryEvent::DeviceUpdated(device_info)
                        };

                        let _ = event_tx.send(evt);
                    }
                    ServiceEvent::SearchStarted(_) => {
                        info!("mDNS search started");
                    }
                    ServiceEvent::SearchStopped(_) => {
                        info!("mDNS search stopped");
                    }
                    _ => {}
                }
            }
        });

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
        let daemon = self.daemon.as_ref().ok_or_else(|| Error::NotStarted)?;

        let props = HashMap::from([
            ("type".to_string(), format!("{:?}", device_type).to_lowercase()),
            ("os".to_string(), os_version.to_string()),
            ("ver".to_string(), app_version.to_string()),
        ]);

        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            device_id,
            device_name,
            "",
            port,
            props,
        )
        .map_err(|e| Error::Mdns(e.to_string()))?;

        daemon
            .register(service_info)
            .map_err(|e| Error::Mdns(e.to_string()))?;

        info!("Registered mDNS service: {} at port {}", device_name, port);

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
