//! Storage module for persistent data
//! SQLite-based storage for pairing records and configuration

pub mod db;
pub mod error;

pub use db::{Database, DeviceType, PairedDevice};
pub use error::{Error, Result};

/// Storage service for device data persistence
pub struct StorageService {
    db: Database,
}

impl StorageService {
    /// Create a new storage service with the given database path
    pub async fn new() -> Result<Self> {
        let db_path = std::path::PathBuf::from(".mmc/storage.db");
        Ok(Self {
            db: Database::open(&db_path).await?,
        })
    }

    /// Create a new storage service with a custom database path
    pub async fn with_path(path: &std::path::Path) -> Result<Self> {
        Ok(Self {
            db: Database::open(path).await?,
        })
    }

    /// Save or update a paired device
    pub async fn save_device(&self, device: &PairedDevice) -> Result<()> {
        self.db.save_paired_device(device).await
    }

    /// Get a paired device by ID
    pub async fn get_device(&self, device_id: &str) -> Result<Option<PairedDevice>> {
        self.db.get_paired_device(device_id).await
    }

    /// List all paired devices
    pub async fn list_devices(&self) -> Result<Vec<PairedDevice>> {
        self.db.list_paired_devices().await
    }

    /// Remove a paired device
    pub async fn remove_device(&self, device_id: &str) -> Result<()> {
        self.db.remove_paired_device(device_id).await
    }

    /// Update last connected timestamp
    pub async fn update_last_connected(&self, device_id: &str) -> Result<()> {
        self.db.update_last_connected(device_id).await
    }

    /// Save a config value
    pub async fn save_config(&self, key: &str, value: &str) -> Result<()> {
        self.db.save_config(key, value).await
    }

    /// Get a config value
    pub async fn get_config(&self, key: &str) -> Result<Option<String>> {
        self.db.get_config(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_storage_service_creation() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let service = StorageService::with_path(&db_path).await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_storage_operations() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let service = StorageService::with_path(&db_path).await.unwrap();

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

        service.save_device(&device).await.unwrap();

        // Retrieve
        let retrieved = service.get_device("device-123").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().device_name, "Test Phone");

        // List
        let devices = service.list_devices().await.unwrap();
        assert_eq!(devices.len(), 1);

        // Remove
        service.remove_device("device-123").await.unwrap();
        let devices = service.list_devices().await.unwrap();
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn test_config_operations() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let service = StorageService::with_path(&db_path).await.unwrap();

        service.save_config("device_name", "My Device").await.unwrap();
        let value = service.get_config("device_name").await.unwrap();
        assert_eq!(value, Some("My Device".to_string()));
    }
}
