//! SQLite database implementation for persistent storage

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
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

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "unknown"),
            Self::Phone => write!(f, "phone"),
            Self::Tablet => write!(f, "tablet"),
            Self::Pc => write!(f, "pc"),
            Self::Tv => write!(f, "tv"),
            Self::Wearable => write!(f, "wearable"),
        }
    }
}

/// Paired device record
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Database wrapper
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Open or create a database at the given path
    pub async fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(|e| Error::Open(e.to_string()))?;

        // Initialize schema
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS paired_devices (
                device_id TEXT PRIMARY KEY,
                device_name TEXT NOT NULL,
                device_type TEXT NOT NULL,
                os_version TEXT,
                app_version TEXT,
                ip_address TEXT,
                port INTEGER,
                public_key_fingerprint TEXT,
                paired_at INTEGER NOT NULL,
                last_connected_at INTEGER,
                trust_level INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_paired_devices_last_connected
            ON paired_devices(last_connected_at);
            "#,
        )
        .map_err(|e| Error::Query(e.to_string()))?;

        info!("Database opened at {:?}", path);

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Save or update a paired device
    pub async fn save_paired_device(&self, device: &PairedDevice) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            r#"
            INSERT INTO paired_devices (
                device_id, device_name, device_type, os_version, app_version,
                ip_address, port, public_key_fingerprint, paired_at, last_connected_at, trust_level
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(device_id) DO UPDATE SET
                device_name = excluded.device_name,
                device_type = excluded.device_type,
                os_version = excluded.os_version,
                app_version = excluded.app_version,
                ip_address = excluded.ip_address,
                port = excluded.port,
                public_key_fingerprint = excluded.public_key_fingerprint,
                last_connected_at = excluded.last_connected_at,
                trust_level = excluded.trust_level
            "#,
            params![
                device.device_id,
                device.device_name,
                device.device_type.to_string(),
                device.os_version,
                device.app_version,
                device.ip_address,
                device.port,
                device.public_key_fingerprint,
                device.paired_at,
                device.last_connected_at,
                device.trust_level,
            ],
        )
        .map_err(|e| Error::Query(e.to_string()))?;

        debug!("Saved paired device: {}", device.device_id);
        Ok(())
    }

    /// Get a paired device by ID
    pub async fn get_paired_device(&self, device_id: &str) -> Result<Option<PairedDevice>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT device_id, device_name, device_type, os_version, app_version,
                       ip_address, port, public_key_fingerprint, paired_at, last_connected_at, trust_level
                FROM paired_devices
                WHERE device_id = ?1
                "#,
            )
            .map_err(|e| Error::Query(e.to_string()))?;

        let result = stmt
            .query_row(params![device_id], |row| {
                Ok(PairedDevice {
                    device_id: row.get(0)?,
                    device_name: row.get(1)?,
                    device_type: DeviceType::from(row.get::<_, String>(2)?.as_str()),
                    os_version: row.get(3)?,
                    app_version: row.get(4)?,
                    ip_address: row.get(5)?,
                    port: row.get(6)?,
                    public_key_fingerprint: row.get(7)?,
                    paired_at: row.get(8)?,
                    last_connected_at: row.get(9)?,
                    trust_level: row.get(10)?,
                })
            })
            .optional()
            .map_err(|e| Error::Query(e.to_string()))?;

        Ok(result)
    }

    /// List all paired devices
    pub async fn list_paired_devices(&self) -> Result<Vec<PairedDevice>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare(
                r#"
                SELECT device_id, device_name, device_type, os_version, app_version,
                       ip_address, port, public_key_fingerprint, paired_at, last_connected_at, trust_level
                FROM paired_devices
                ORDER BY last_connected_at DESC
                "#,
            )
            .map_err(|e| Error::Query(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(PairedDevice {
                    device_id: row.get(0)?,
                    device_name: row.get(1)?,
                    device_type: DeviceType::from(row.get::<_, String>(2)?.as_str()),
                    os_version: row.get(3)?,
                    app_version: row.get(4)?,
                    ip_address: row.get(5)?,
                    port: row.get(6)?,
                    public_key_fingerprint: row.get(7)?,
                    paired_at: row.get(8)?,
                    last_connected_at: row.get(9)?,
                    trust_level: row.get(10)?,
                })
            })
            .map_err(|e| Error::Query(e.to_string()))?;

        let devices: Vec<PairedDevice> = rows.filter_map(|r| r.ok()).collect();
        Ok(devices)
    }

    /// Remove a paired device
    pub async fn remove_paired_device(&self, device_id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute(
            "DELETE FROM paired_devices WHERE device_id = ?1",
            params![device_id],
        )
        .map_err(|e| Error::Query(e.to_string()))?;

        debug!("Removed paired device: {}", device_id);
        Ok(())
    }

    /// Update last connected timestamp
    pub async fn update_last_connected(&self, device_id: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE paired_devices SET last_connected_at = ?1 WHERE device_id = ?2",
            params![now, device_id],
        )
        .map_err(|e| Error::Query(e.to_string()))?;

        Ok(())
    }

    /// Save a config value
    pub async fn save_config(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            r#"
            INSERT INTO config (key, value, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at
            "#,
            params![key, value, now],
        )
        .map_err(|e| Error::Query(e.to_string()))?;

        Ok(())
    }

    /// Get a config value
    pub async fn get_config(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare("SELECT value FROM config WHERE key = ?1")
            .map_err(|e| Error::Query(e.to_string()))?;

        let result = stmt
            .query_row(params![key], |row| row.get(0))
            .optional()
            .map_err(|e| Error::Query(e.to_string()))?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_database_operations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let db = Database::open(&db_path).await.unwrap();

        // Save a device
        let device = PairedDevice {
            device_id: "device-123".to_string(),
            device_name: "Test Phone".to_string(),
            device_type: DeviceType::Phone,
            os_version: "Android 13".to_string(),
            app_version: "1.0.0".to_string(),
            ip_address: "192.168.1.100".to_string(),
            port: 8080,
            public_key_fingerprint: "abc123".to_string(),
            paired_at: chrono::Utc::now().timestamp(),
            last_connected_at: None,
            trust_level: 1,
        };

        db.save_paired_device(&device).await.unwrap();

        // Retrieve
        let retrieved = db.get_paired_device("device-123").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.device_name, "Test Phone");

        // List
        let devices = db.list_paired_devices().await.unwrap();
        assert_eq!(devices.len(), 1);

        // Remove
        db.remove_paired_device("device-123").await.unwrap();
        let devices = db.list_paired_devices().await.unwrap();
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn test_update_last_connected() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let db = Database::open(&db_path).await.unwrap();

        // Save a device
        let device = PairedDevice {
            device_id: "device-456".to_string(),
            device_name: "Test Tablet".to_string(),
            device_type: DeviceType::Tablet,
            os_version: "iOS 17".to_string(),
            app_version: "1.0.0".to_string(),
            ip_address: "192.168.1.200".to_string(),
            port: 9090,
            public_key_fingerprint: "xyz789".to_string(),
            paired_at: chrono::Utc::now().timestamp(),
            last_connected_at: None,
            trust_level: 1,
        };

        db.save_paired_device(&device).await.unwrap();

        // Update last connected
        db.update_last_connected("device-456").await.unwrap();

        // Retrieve and check
        let retrieved = db.get_paired_device("device-456").await.unwrap();
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert!(retrieved.last_connected_at.is_some());
        assert!(retrieved.last_connected_at.unwrap() > 0);
    }

    #[tokio::test]
    async fn test_multiple_devices() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let db = Database::open(&db_path).await.unwrap();

        // Save multiple devices
        for i in 0..3 {
            let device = PairedDevice {
                device_id: format!("device-{}", i),
                device_name: format!("Device {}", i),
                device_type: DeviceType::Phone,
                os_version: "Android 13".to_string(),
                app_version: "1.0.0".to_string(),
                ip_address: format!("192.168.1.{}", 100 + i),
                port: 8080 + i as u16,
                public_key_fingerprint: format!("fingerprint-{}", i),
                paired_at: chrono::Utc::now().timestamp(),
                last_connected_at: None,
                trust_level: 1,
            };
            db.save_paired_device(&device).await.unwrap();
        }

        // Count
        let devices = db.list_paired_devices().await.unwrap();
        assert_eq!(devices.len(), 3);

        // Individual lookups
        for i in 0..3 {
            let id = format!("device-{}", i);
            let retrieved = db.get_paired_device(&id).await.unwrap();
            assert!(retrieved.is_some());
        }
    }

    #[tokio::test]
    async fn test_get_nonexistent_device() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).await.unwrap();

        let result = db.get_paired_device("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_device_type_variants() {
        assert_eq!(DeviceType::from("phone"), DeviceType::Phone);
        assert_eq!(DeviceType::from("tablet"), DeviceType::Tablet);
        assert_eq!(DeviceType::from("tv"), DeviceType::Tv);
        assert_eq!(DeviceType::from("wearable"), DeviceType::Wearable);
    }

    #[tokio::test]
    async fn test_config_operations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let db = Database::open(&db_path).await.unwrap();

        // Missing config
        let missing = db.get_config("missing").await.unwrap();
        assert!(missing.is_none());

        // Save and get
        db.save_config("device_name", "My Device").await.unwrap();
        let value = db.get_config("device_name").await.unwrap();
        assert_eq!(value, Some("My Device".to_string()));

        // Update
        db.save_config("device_name", "Updated Device").await.unwrap();
        let value = db.get_config("device_name").await.unwrap();
        assert_eq!(value, Some("Updated Device".to_string()));
    }
}
