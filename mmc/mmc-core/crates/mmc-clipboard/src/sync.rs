//! Clipboard synchronization between devices.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, warn};
use mmc_protocol::{ClipboardContent, ClipboardData};
use mmc_transport::ConnectionManager;

use crate::error::{ClipboardError, Result};
use crate::manager::{ClipboardEntry, ClipboardManager, ClipboardSource, clipboard_data_size, ClipboardManagerEvent};

/// Sync direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncDirection {
    /// Push only (local to remote).
    Push,
    /// Pull only (remote to local).
    Pull,
    /// Bidirectional sync.
    Bidirectional,
}

/// Sync event emitted by the syncer.
#[derive(Debug, Clone)]
pub enum SyncEvent {
    /// Content pushed to remote.
    Pushed(ClipboardEntry),
    /// Content pulled from remote.
    Pulled(ClipboardEntry),
    /// Sync error.
    Error(String),
    /// Connected to remote.
    Connected(String),
    /// Disconnected.
    Disconnected(String),
}

/// Clipboard sync configuration.
#[derive(Debug, Clone)]
pub struct ClipboardConfig {
    /// Sync direction.
    pub direction: SyncDirection,
    /// Sync interval in milliseconds.
    pub sync_interval_ms: u64,
    /// Whether to sync images.
    pub sync_images: bool,
    /// Whether to sync URLs.
    pub sync_urls: bool,
    /// Max content size in bytes.
    pub max_content_size: usize,
    /// Whether to auto-sync.
    pub auto_sync: bool,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            direction: SyncDirection::Bidirectional,
            sync_interval_ms: 1000,
            sync_images: true,
            sync_urls: true,
            max_content_size: 10 * 1024 * 1024,
            auto_sync: true,
        }
    }
}

/// Clipboard syncer for synchronizing clipboard across devices.
pub struct ClipboardSyncer {
    /// Local clipboard manager.
    manager: Arc<ClipboardManager>,
    /// Sync configuration.
    config: ClipboardConfig,
    /// Connection manager.
    connection_manager: Arc<ConnectionManager>,
    /// Connected remote device IDs.
    connected_remotes: Arc<RwLock<HashMap<String, RemoteConnection>>>,
    /// Last sent hashes (to avoid resending).
    last_sent_hashes: Arc<RwLock<HashMap<String, u64>>>,
    /// Event channel.
    event_tx: mpsc::UnboundedSender<SyncEvent>,
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<SyncEvent>>>,
    /// Whether syncer is running.
    running: Arc<std::sync::atomic::AtomicBool>,
    /// Last sync timestamp.
    last_sync_ms: Arc<Mutex<u64>>,
    /// Self weak reference for spawning tasks.
    self_weak: std::sync::Weak<Self>,
}

/// Connection to a remote device for clipboard sync.
struct RemoteConnection {
    device_id: String,
    last_pushed_hash: u64,
    last_pulled_hash: u64,
    sender: mpsc::UnboundedSender<ClipboardContent>,
}

impl ClipboardSyncer {
    /// Create a new clipboard syncer.
    pub fn new(
        manager: Arc<ClipboardManager>,
        connection_manager: Arc<ConnectionManager>,
        config: ClipboardConfig,
    ) -> Arc<Self> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        
        let syncer = Arc::new_cyclic(|weak| Self {
            manager,
            config,
            connection_manager,
            connected_remotes: Arc::new(RwLock::new(HashMap::new())),
            last_sent_hashes: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            last_sync_ms: Arc::new(Mutex::new(0)),
            self_weak: weak.clone(),
        });
        
        syncer
    }
    
    /// Create with default configuration.
    pub fn with_defaults(
        manager: Arc<ClipboardManager>,
        connection_manager: Arc<ConnectionManager>,
    ) -> Arc<Self> {
        Self::new(manager, connection_manager, ClipboardConfig::default())
    }
    
    /// Get the local manager.
    pub fn manager(&self) -> &Arc<ClipboardManager> {
        &self.manager
    }
    
    /// Get the config.
    pub fn config(&self) -> &ClipboardConfig {
        &self.config
    }
    
    /// Add a remote device for syncing.
    pub async fn add_remote(&self, device_id: String) {
        let mut remotes = self.connected_remotes.write().await;
        remotes.insert(
            device_id.clone(),
            RemoteConnection {
                device_id: device_id.clone(),
                last_pushed_hash: 0,
                last_pulled_hash: 0,
                sender: mpsc::unbounded_channel().0,
            },
        );
        
        let _ = self.event_tx.send(SyncEvent::Connected(device_id));
    }
    
    /// Remove a remote device.
    pub async fn remove_remote(&self, device_id: &str) {
        let mut remotes = self.connected_remotes.write().await;
        if remotes.remove(device_id).is_some() {
            let _ = self.event_tx.send(SyncEvent::Disconnected(device_id.to_string()));
        }
    }
    
    /// Get list of connected remotes.
    pub async fn connected_remotes(&self) -> Vec<String> {
        self.connected_remotes.read().await.keys().cloned().collect()
    }
    
    /// Check if running.
    pub fn is_running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }
    
    /// Start the sync loop.
    pub async fn start(&self) -> Result<()> {
        if self.is_running() {
            return Ok(());
        }
        
        self.running.store(true, std::sync::atomic::Ordering::SeqCst);
        
        // Spawn task using weak reference to self
        let weak = self.self_weak.clone();
        tokio::spawn(async move {
            if let Some(syncer_arc) = weak.upgrade() {
                syncer_arc.run_loop_impl().await;
            }
        });
        
        Ok(())
    }
    
    /// Stop the sync loop.
    pub async fn stop(&self) -> Result<()> {
        self.running.store(false, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
    
    /// Manually trigger a sync.
    pub async fn sync_now(&self) -> Result<()> {
        self.do_sync().await
    }
    
    /// Receive a clipboard content from remote.
    pub async fn receive_content(&self, content: ClipboardContent) -> Result<()> {
        // Check size
        let size = crate::manager::clipboard_data_size(&content.content);
        if size > self.config.max_content_size {
            return Err(ClipboardError::ContentTooLarge(size, self.config.max_content_size));
        }
        
        // Check if should sync this type
        match &content.content {
            ClipboardData::Image { .. } if !self.config.sync_images => {
                return Ok(()); // Skip
            }
            ClipboardData::Url { .. } if !self.config.sync_urls => {
                return Ok(());
            }
            _ => {}
        }
        
        // Set as remote content
        let entry = ClipboardEntry::new(content.clone(), ClipboardSource::Remote);
        self.manager.set_remote(content).await?;
        
        let _ = self.event_tx.send(SyncEvent::Pulled(entry));
        
        Ok(())
    }
    
    /// Try to receive a sync event.
    pub async fn try_recv_event(&self) -> Option<SyncEvent> {
        let mut rx = self.event_rx.lock().await;
        rx.try_recv().ok()
    }
    
    /// Get last sync timestamp.
    pub async fn last_sync(&self) -> u64 {
        *self.last_sync_ms.lock().await
    }
    
    /// Run the sync loop.
    async fn run_loop_impl(self: Arc<Self>) {
        let mut ticker = interval(Duration::from_millis(self.config.sync_interval_ms));
        
        while self.is_running() {
            ticker.tick().await;
            
            if !self.is_running() {
                break;
            }
            
            if self.config.auto_sync {
                if let Err(e) = self.do_sync().await {
                    warn!("Sync failed: {}", e);
                    let _ = self.event_tx.send(SyncEvent::Error(e.to_string()));
                }
            }
        }
    }
    
    /// Perform sync.
    async fn do_sync(&self) -> Result<()> {
        let remotes = self.connected_remotes.read().await;
        if remotes.is_empty() {
            return Ok(());
        }
        
        let current = self.manager.get_current().await;
        
        if let Some(entry) = current {
            // Check direction
            if self.config.direction == SyncDirection::Push 
                || self.config.direction == SyncDirection::Bidirectional {
                self.push_to_remotes(&entry).await?;
            }
        }
        
        // Update timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        *self.last_sync_ms.lock().await = now;
        
        Ok(())
    }
    
    /// Push content to all remotes.
    async fn push_to_remotes(&self, entry: &ClipboardEntry) -> Result<()> {
        let device_ids: Vec<String> = self.connected_remotes.read().await.keys().cloned().collect();
        
        for device_id in device_ids {
            // Check if we already sent this content
            let last_hash = {
                let sent = self.last_sent_hashes.read().await;
                sent.get(&device_id).copied().unwrap_or(0)
            };
            
            if last_hash == entry.hash {
                continue; // Skip, already sent
            }
            
            // Send the content
            debug!("Pushing clipboard to {}", device_id);
            
            // Mark as sent
            let mut sent = self.last_sent_hashes.write().await;
            sent.insert(device_id.clone(), entry.hash);
            
            let _ = self.event_tx.send(SyncEvent::Pushed(entry.clone()));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    
    #[test]
    fn test_sync_direction_eq() {
        assert_eq!(SyncDirection::Push, SyncDirection::Push);
        assert_ne!(SyncDirection::Push, SyncDirection::Pull);
    }
    
    #[test]
    fn test_clipboard_config_default() {
        let config = ClipboardConfig::default();
        assert_eq!(config.direction, SyncDirection::Bidirectional);
        assert_eq!(config.sync_interval_ms, 1000);
        assert!(config.sync_images);
        assert!(config.sync_urls);
        assert!(config.auto_sync);
    }
    
    #[tokio::test]
    async fn test_clipboard_syncer_new() {
        let manager = Arc::new(ClipboardManager::new());
        let conn_manager = Arc::new(ConnectionManager::default_config());
        let syncer = ClipboardSyncer::with_defaults(manager, conn_manager);
        
        assert!(!syncer.is_running());
        assert_eq!(syncer.connected_remotes().await.len(), 0);
    }
    
    #[tokio::test]
    async fn test_clipboard_syncer_add_remove_remote() {
        let manager = Arc::new(ClipboardManager::new());
        let conn_manager = Arc::new(ConnectionManager::default_config());
        let syncer = ClipboardSyncer::with_defaults(manager, conn_manager);
        
        syncer.add_remote("device-1".to_string()).await;
        assert_eq!(syncer.connected_remotes().await.len(), 1);
        
        syncer.remove_remote("device-1").await;
        assert_eq!(syncer.connected_remotes().await.len(), 0);
    }
    
    #[tokio::test]
    async fn test_clipboard_syncer_start_stop() {
        let manager = Arc::new(ClipboardManager::new());
        let conn_manager = Arc::new(ConnectionManager::default_config());
        let syncer = ClipboardSyncer::with_defaults(manager, conn_manager);
        
        syncer.start().await.unwrap();
        assert!(syncer.is_running());
        
        syncer.stop().await.unwrap();
        assert!(!syncer.is_running());
    }
    
    #[tokio::test]
    async fn test_clipboard_syncer_receive_content() {
        let manager = Arc::new(ClipboardManager::new());
        let conn_manager = Arc::new(ConnectionManager::default_config());
        let syncer = ClipboardSyncer::with_defaults(manager.clone(), conn_manager);
        
        let content = ClipboardContent {
            timestamp_ms: 1000,
            content: ClipboardData::Text { text: "Remote content".to_string() },
        };
        
        syncer.receive_content(content).await.unwrap();
        
        // Verify it's in the manager
        let current = manager.get_current().await;
        assert!(current.is_some());
        assert_eq!(current.unwrap().source, ClipboardSource::Remote);
    }
    
    #[tokio::test]
    async fn test_clipboard_syncer_receive_too_large() {
        let config = ClipboardConfig {
            max_content_size: 5,
            ..Default::default()
        };
        let manager = Arc::new(ClipboardManager::new());
        let conn_manager = Arc::new(ConnectionManager::default_config());
        let syncer = ClipboardSyncer::new(manager, conn_manager, config);
        
        let content = ClipboardContent {
            timestamp_ms: 1000,
            content: ClipboardData::Text { text: "Way too long content".to_string() },
        };
        
        let result = syncer.receive_content(content).await;
        assert!(matches!(result, Err(ClipboardError::ContentTooLarge(_, 5))));
    }
    
    #[tokio::test]
    async fn test_clipboard_syncer_skip_images() {
        let config = ClipboardConfig {
            sync_images: false,
            ..Default::default()
        };
        let manager = Arc::new(ClipboardManager::new());
        let conn_manager = Arc::new(ConnectionManager::default_config());
        let syncer = ClipboardSyncer::new(manager.clone(), conn_manager, config);
        
        let content = ClipboardContent {
            timestamp_ms: 1000,
            content: ClipboardData::Image { image_png: vec![1, 2, 3] },
        };
        
        syncer.receive_content(content).await.unwrap();
        
        // Should be skipped (not stored)
        let current = manager.get_current().await;
        assert!(current.is_none());
    }
    
    #[tokio::test]
    async fn test_clipboard_syncer_sync_now() {
        let manager = Arc::new(ClipboardManager::new());
        let conn_manager = Arc::new(ConnectionManager::default_config());
        let syncer = ClipboardSyncer::with_defaults(manager.clone(), conn_manager);
        
        // Set local content
        let content = ClipboardContent {
            timestamp_ms: 1000,
            content: ClipboardData::Text { text: "Sync me".to_string() },
        };
        manager.set_local(content).await.unwrap();
        
        // Add a remote
        syncer.add_remote("device-1".to_string()).await;
        
        // Trigger sync
        syncer.sync_now().await.unwrap();
        
        let last_sync = syncer.last_sync().await;
        assert!(last_sync > 0);
    }
    
    #[tokio::test]
    async fn test_clipboard_syncer_event() {
        let manager = Arc::new(ClipboardManager::new());
        let conn_manager = Arc::new(ConnectionManager::default_config());
        let syncer = ClipboardSyncer::with_defaults(manager, conn_manager);
        
        // Add a remote - should emit Connected event
        syncer.add_remote("device-1".to_string()).await;
        
        // Try to receive event
        let event = syncer.try_recv_event().await;
        assert!(matches!(event, Some(SyncEvent::Connected(_))));
    }
}