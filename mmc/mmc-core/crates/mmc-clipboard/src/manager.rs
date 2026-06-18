//! Clipboard manager for local clipboard state management.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, Mutex, RwLock};
use mmc_protocol::{ClipboardContent, ClipboardData};

use crate::error::{ClipboardError, Result};

/// Compute the size of clipboard data in bytes.
pub fn clipboard_data_size(data: &ClipboardData) -> usize {
    match data {
        ClipboardData::Text { text } => text.len(),
        ClipboardData::Image { image_png } => image_png.len(),
        ClipboardData::Url { url } => url.len(),
    }
}

/// Source of clipboard content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardSource {
    /// Set by local user action.
    Local,
    /// Received from a remote device.
    Remote,
    /// Set by the system.
    System,
}

/// A single clipboard entry with metadata.
#[derive(Debug, Clone)]
pub struct ClipboardEntry {
    pub content: ClipboardContent,
    pub source: ClipboardSource,
    pub timestamp_ms: u64,
    pub content_size: usize,
    pub hash: u64,
}

impl ClipboardEntry {
    /// Create a new clipboard entry.
    pub fn new(content: ClipboardContent, source: ClipboardSource) -> Self {
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let content_size = clipboard_data_size(&content.content);
        let hash = Self::compute_hash(&content);
        
        Self {
            content,
            source,
            timestamp_ms,
            content_size,
            hash,
        }
    }
    
    /// Compute a simple hash of clipboard content.
    pub fn compute_hash(content: &ClipboardContent) -> u64 {
        let mut hasher: u64 = 14695981039346656037; // FNV offset basis
        let prime: u64 = 1099511628211;
        
        match &content.content {
            ClipboardData::Text { text } => {
                for b in text.as_bytes() {
                    hasher ^= *b as u64;
                    hasher = hasher.wrapping_mul(prime);
                }
            }
            ClipboardData::Image { image_png } => {
                for b in image_png {
                    hasher ^= *b as u64;
                    hasher = hasher.wrapping_mul(prime);
                }
            }
            ClipboardData::Url { url } => {
                for b in url.as_bytes() {
                    hasher ^= *b as u64;
                    hasher = hasher.wrapping_mul(prime);
                }
            }
        }
        
        hasher
    }
}

/// Event emitted by the clipboard manager.
#[derive(Debug, Clone)]
pub enum ClipboardManagerEvent {
    /// New local content set.
    LocalSet(ClipboardEntry),
    /// Content received from remote.
    RemoteReceived(ClipboardEntry),
    /// Content cleared.
    Cleared,
}

/// Clipboard manager for managing local clipboard state.
pub struct ClipboardManager {
    /// Current clipboard content.
    current: Arc<RwLock<Option<ClipboardEntry>>>,
    /// History of recent clipboard entries.
    history: Arc<RwLock<Vec<ClipboardEntry>>>,
    /// Event channel.
    event_tx: mpsc::UnboundedSender<ClipboardManagerEvent>,
    /// Event receiver.
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<ClipboardManagerEvent>>>,
    /// Max history size.
    max_history: usize,
    /// Max content size in bytes.
    max_content_size: usize,
}

impl ClipboardManager {
    /// Create a new clipboard manager.
    pub fn new() -> Self {
        Self::with_limits(50, 10 * 1024 * 1024)
    }
    
    /// Create with custom limits.
    pub fn with_limits(max_history: usize, max_content_size: usize) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        
        Self {
            current: Arc::new(RwLock::new(None)),
            history: Arc::new(RwLock::new(Vec::new())),
            event_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            max_history,
            max_content_size,
        }
    }
    
    /// Set local clipboard content.
    pub async fn set_local(&self, content: ClipboardContent) -> Result<()> {
        // Check size
        let size = clipboard_data_size(&content.content);
        if size > self.max_content_size {
            return Err(ClipboardError::ContentTooLarge(size, self.max_content_size));
        }
        
        let entry = ClipboardEntry::new(content, ClipboardSource::Local);
        
        // Update current
        {
            let mut current = self.current.write().await;
            *current = Some(entry.clone());
        }
        
        // Add to history
        {
            let mut history = self.history.write().await;
            history.push(entry.clone());
            if history.len() > self.max_history {
                history.remove(0);
            }
        }
        
        // Notify
        let _ = self.event_tx.send(ClipboardManagerEvent::LocalSet(entry));
        
        Ok(())
    }
    
    /// Set clipboard content received from remote.
    pub async fn set_remote(&self, content: ClipboardContent) -> Result<()> {
        let size = clipboard_data_size(&content.content);
        if size > self.max_content_size {
            return Err(ClipboardError::ContentTooLarge(size, self.max_content_size));
        }
        
        let entry = ClipboardEntry::new(content, ClipboardSource::Remote);
        
        {
            let mut current = self.current.write().await;
            *current = Some(entry.clone());
        }
        
        {
            let mut history = self.history.write().await;
            history.push(entry.clone());
            if history.len() > self.max_history {
                history.remove(0);
            }
        }
        
        let _ = self.event_tx.send(ClipboardManagerEvent::RemoteReceived(entry));
        
        Ok(())
    }
    
    /// Get current clipboard content.
    pub async fn get_current(&self) -> Option<ClipboardEntry> {
        self.current.read().await.clone()
    }
    
    /// Get clipboard history.
    pub async fn get_history(&self) -> Vec<ClipboardEntry> {
        self.history.read().await.clone()
    }
    
    /// Clear the clipboard.
    pub async fn clear(&self) -> Result<()> {
        let mut current = self.current.write().await;
        *current = None;
        
        let _ = self.event_tx.send(ClipboardManagerEvent::Cleared);
        Ok(())
    }
    
    /// Check if clipboard has content.
    pub async fn is_empty(&self) -> bool {
        self.current.read().await.is_none()
    }
    
    /// Get history size.
    pub async fn history_size(&self) -> usize {
        self.history.read().await.len()
    }
    
    /// Try to receive an event.
    pub async fn try_recv_event(&self) -> Option<ClipboardManagerEvent> {
        let mut rx = self.event_rx.lock().await;
        rx.try_recv().ok()
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_clipboard_source_eq() {
        assert_eq!(ClipboardSource::Local, ClipboardSource::Local);
        assert_ne!(ClipboardSource::Local, ClipboardSource::Remote);
    }
    
    #[test]
    fn test_clipboard_data_size_text() {
        let data = ClipboardData::Text { text: "Hello".to_string() };
        assert_eq!(clipboard_data_size(&data), 5);
    }
    
    #[test]
    fn test_clipboard_data_size_image() {
        let data = ClipboardData::Image { image_png: vec![0u8; 100] };
        assert_eq!(clipboard_data_size(&data), 100);
    }
    
    #[test]
    fn test_clipboard_data_size_url() {
        let data = ClipboardData::Url { url: "https://example.com".to_string() };
        assert_eq!(clipboard_data_size(&data), 19);
    }
    
    #[tokio::test]
    async fn test_clipboard_manager_new() {
        let manager = ClipboardManager::new();
        assert!(manager.is_empty().await);
        assert_eq!(manager.history_size().await, 0);
    }
    
    #[tokio::test]
    async fn test_clipboard_manager_set_local() {
        let manager = ClipboardManager::new();
        let content = ClipboardContent {
            timestamp_ms: 1000,
            content: ClipboardData::Text { text: "Test".to_string() },
        };
        
        manager.set_local(content).await.unwrap();
        
        let current = manager.get_current().await;
        assert!(current.is_some());
        assert_eq!(current.unwrap().content_size, 4);
    }
    
    #[tokio::test]
    async fn test_clipboard_manager_set_remote() {
        let manager = ClipboardManager::new();
        let content = ClipboardContent {
            timestamp_ms: 2000,
            content: ClipboardData::Url { url: "https://test.com".to_string() },
        };
        
        manager.set_remote(content).await.unwrap();
        
        let current = manager.get_current().await;
        assert!(current.is_some());
        assert_eq!(current.unwrap().source, ClipboardSource::Remote);
    }
    
    #[tokio::test]
    async fn test_clipboard_manager_content_too_large() {
        let manager = ClipboardManager::with_limits(50, 10);
        let content = ClipboardContent {
            timestamp_ms: 1000,
            content: ClipboardData::Text { text: "This is way too long".to_string() },
        };
        
        let result = manager.set_local(content).await;
        assert!(matches!(result, Err(ClipboardError::ContentTooLarge(_, 10))));
    }
    
    #[tokio::test]
    async fn test_clipboard_manager_clear() {
        let manager = ClipboardManager::new();
        let content = ClipboardContent {
            timestamp_ms: 1000,
            content: ClipboardData::Text { text: "Test".to_string() },
        };
        
        manager.set_local(content).await.unwrap();
        assert!(!manager.is_empty().await);
        
        manager.clear().await.unwrap();
        assert!(manager.is_empty().await);
    }
    
    #[tokio::test]
    async fn test_clipboard_manager_history() {
        let manager = ClipboardManager::new();
        
        for i in 0..3 {
            let content = ClipboardContent {
                timestamp_ms: i as u64,
                content: ClipboardData::Text { text: format!("Text {}", i) },
            };
            manager.set_local(content).await.unwrap();
        }
        
        let history = manager.get_history().await;
        assert_eq!(history.len(), 3);
    }
    
    #[tokio::test]
    async fn test_clipboard_manager_history_max() {
        let manager = ClipboardManager::with_limits(2, 10 * 1024 * 1024);
        
        for i in 0..5 {
            let content = ClipboardContent {
                timestamp_ms: i as u64,
                content: ClipboardData::Text { text: format!("T{}", i) },
            };
            manager.set_local(content).await.unwrap();
        }
        
        let history = manager.get_history().await;
        assert_eq!(history.len(), 2);
    }
    
    #[tokio::test]
    async fn test_clipboard_manager_event() {
        let manager = ClipboardManager::new();
        let content = ClipboardContent {
            timestamp_ms: 1000,
            content: ClipboardData::Text { text: "Hello".to_string() },
        };
        
        manager.set_local(content).await.unwrap();
        
        let event = manager.try_recv_event().await;
        assert!(matches!(event, Some(ClipboardManagerEvent::LocalSet(_))));
    }
    
    #[test]
    fn test_clipboard_entry_hash_different() {
        let content1 = ClipboardContent {
            timestamp_ms: 1000,
            content: ClipboardData::Text { text: "A".to_string() },
        };
        let content2 = ClipboardContent {
            timestamp_ms: 1000,
            content: ClipboardData::Text { text: "B".to_string() },
        };
        
        let hash1 = ClipboardEntry::compute_hash(&content1);
        let hash2 = ClipboardEntry::compute_hash(&content2);
        
        assert_ne!(hash1, hash2);
    }
    
    #[test]
    fn test_clipboard_entry_hash_same() {
        let content1 = ClipboardContent {
            timestamp_ms: 1000,
            content: ClipboardData::Text { text: "Hello".to_string() },
        };
        let content2 = ClipboardContent {
            timestamp_ms: 2000,
            content: ClipboardData::Text { text: "Hello".to_string() },
        };
        
        let hash1 = ClipboardEntry::compute_hash(&content1);
        let hash2 = ClipboardEntry::compute_hash(&content2);
        
        assert_eq!(hash1, hash2);
    }
    
    #[test]
    fn test_clipboard_entry_new() {
        let content = ClipboardContent {
            timestamp_ms: 1000,
            content: ClipboardData::Text { text: "Test".to_string() },
        };
        let entry = ClipboardEntry::new(content.clone(), ClipboardSource::Local);
        
        assert_eq!(entry.content_size, 4);
        assert_eq!(entry.source, ClipboardSource::Local);
        assert!(entry.timestamp_ms > 0);
    }
}