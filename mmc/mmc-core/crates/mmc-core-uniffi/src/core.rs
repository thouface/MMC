//! MMC Core - Main entry point for the library
//!
//! This module provides the main `MmcCore` struct that orchestrates
//! all subsystems: discovery, pairing, file transfer, and storage.

use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::error::{CoreError, Result};
use crate::types::{CoreConfig, DeviceInfo, TransferProgress};

/// MMC Core instance
/// This is the main entry point for using MMC functionality
pub struct MmcCore {
    config: RwLock<Option<CoreConfig>>,
    initialized: RwLock<bool>,
}

impl MmcCore {
    /// Create a new MmcCore instance
    pub fn new() -> Self {
        Self {
            config: RwLock::new(None),
            initialized: RwLock::new(false),
        }
    }

    /// Initialize the core with configuration
    pub async fn init(&self, config: CoreConfig) -> Result<()> {
        // Check not already initialized
        {
            let initialized = self.initialized.read().await;
            if *initialized {
                return Err(CoreError::AlreadyInitialized);
            }
        }

        info!("Initializing MMC Core for device: {}", config.device_name);

        // Store config
        {
            let mut cfg = self.config.write().await;
            *cfg = Some(config);
        }

        // Mark as initialized
        {
            let mut initialized = self.initialized.write().await;
            *initialized = true;
        }

        info!("MMC Core initialized successfully");
        Ok(())
    }

    /// Check if core is initialized
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }

    /// Get current configuration
    pub async fn get_config(&self) -> Result<CoreConfig> {
        let config = self.config.read().await;
        config.clone().ok_or(CoreError::NotInitialized)
    }

    /// Shutdown the core
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down MMC Core");
        let mut initialized = self.initialized.write().await;
        *initialized = false;
        Ok(())
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

    #[tokio::test]
    async fn test_core_lifecycle() {
        let core = MmcCore::new();

        // Should not be initialized initially
        assert!(!core.is_initialized().await);

        // Init with default config
        let config = CoreConfig::default();
        assert!(core.init(config).await.is_ok());

        // Should be initialized now
        assert!(core.is_initialized().await);

        // Shutdown
        assert!(core.shutdown().await.is_ok());
        assert!(!core.is_initialized().await);
    }

    #[tokio::test]
    async fn test_double_init_fails() {
        let core = MmcCore::new();

        let config = CoreConfig::default();
        assert!(core.init(config).await.is_ok());

        // Second init should fail
        let config2 = CoreConfig::default();
        assert!(core.init(config2).await.is_err());
    }
}
