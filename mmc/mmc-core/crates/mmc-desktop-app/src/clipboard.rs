//! Platform-specific clipboard support for PC platforms
//!
//! Uses the `arboard` crate for clipboard access on Windows, macOS, and Linux.

use std::sync::Arc;
use std::time::Duration;

use mmc_clipboard::ClipboardManager;
use mmc_protocol::{ClipboardContent, ClipboardData};

#[allow(unused_imports)]
use crate::error::Result;
#[cfg(feature = "clipboard")]
use crate::error::DesktopError;

/// PC clipboard monitor using arboard crate
pub struct PcClipboardMonitor {
    manager: Arc<ClipboardManager>,
    #[cfg(feature = "clipboard")]
    arboard: arboard::Clipboard,
    last_content: Option<String>,
    running: bool,
}

impl PcClipboardMonitor {
    /// Create a new PC clipboard monitor
    pub fn new(manager: Arc<ClipboardManager>) -> Result<Self> {
        #[cfg(feature = "clipboard")]
        let arboard = arboard::Clipboard::new()
            .map_err(|e| DesktopError::Clipboard(e.to_string()))?;

        Ok(Self {
            manager,
            #[cfg(feature = "clipboard")]
            arboard,
            last_content: None,
            running: false,
        })
    }

    /// Start monitoring clipboard for changes
    pub fn start(&mut self) -> Result<()> {
        self.running = true;
        tracing::info!("PC clipboard monitor started");
        Ok(())
    }

    /// Stop monitoring
    pub fn stop(&mut self) -> Result<()> {
        self.running = false;
        tracing::info!("PC clipboard monitor stopped");
        Ok(())
    }

    /// Check if monitor is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get current clipboard text from system
    #[cfg(feature = "clipboard")]
    pub fn get_system_clipboard(&mut self) -> Result<Option<String>> {
        let text = self.arboard.get_text()
            .map_err(|e| DesktopError::Clipboard(e.to_string()))?;
        Ok(Some(text))
    }

    /// Get current clipboard text from system (stub for no-clipboard feature)
    #[cfg(not(feature = "clipboard"))]
    pub fn get_system_clipboard(&mut self) -> Result<Option<String>> {
        Ok(None)
    }

    /// Set clipboard text to system
    #[cfg(feature = "clipboard")]
    pub fn set_system_clipboard(&mut self, text: &str) -> Result<()> {
        self.arboard.set_text(text)
            .map_err(|e| DesktopError::Clipboard(e.to_string()))?;
        self.last_content = Some(text.to_string());
        Ok(())
    }

    /// Set clipboard text to system (stub for no-clipboard feature)
    #[cfg(not(feature = "clipboard"))]
    pub fn set_system_clipboard(&mut self, text: &str) -> Result<()> {
        self.last_content = Some(text.to_string());
        Ok(())
    }

    /// Check for clipboard changes and sync to manager
    pub async fn check_and_sync(&mut self) -> Result<Option<ClipboardContent>> {
        if !self.running {
            return Ok(None);
        }

        let current_text = self.get_system_clipboard()?;
        let last_text = self.last_content.as_deref().unwrap_or_default();
        match current_text {
            Some(text) if text != last_text => {
                self.last_content = Some(text.clone());

                let content = ClipboardContent {
                    timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
                    content: ClipboardData::Text { text },
                };

                self.manager.set_local(content.clone()).await?;
                let len = match &content.content {
                    ClipboardData::Text { text } => text.len(),
                    ClipboardData::Url { url } => url.len(),
                    ClipboardData::Image { image_png } => image_png.len(),
                };
                tracing::debug!("Clipboard synced: {} chars", len);
                Ok(Some(content))
            }
            _ => Ok(None),
        }
    }

    /// Sync from manager to system clipboard
    pub async fn sync_to_system(&mut self) -> Result<()> {
        let current = self.manager.get_current().await;
        let last_text = self.last_content.as_deref().unwrap_or_default();
        match current {
            Some(entry) => match &entry.content.content {
                ClipboardData::Text { text } => {
                    if text != last_text {
                        self.set_system_clipboard(text)?;
                        tracing::debug!("Synced to system clipboard: {} chars", text.len());
                    }
                }
                ClipboardData::Url { url } => {
                    self.set_system_clipboard(url)?;
                }
                ClipboardData::Image { .. } => {
                    // Image clipboard not supported in basic implementation
                    tracing::warn!("Image clipboard sync not implemented");
                }
            },
            None => {}
        }
        Ok(())
    }

    /// Run clipboard monitoring loop
    pub async fn run_monitor_loop(&mut self, interval_ms: u64) -> Result<()> {
        self.start()?;
        while self.running {
            self.check_and_sync().await?;
            tokio::time::sleep(Duration::from_millis(interval_ms)).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests that require arboard clipboard access - only run when clipboard feature is disabled
    // (in CI environments without X11/Wayland, arboard will fail)
    #[cfg(not(feature = "clipboard"))]
    #[test]
    fn test_clipboard_monitor_new() {
        let manager = Arc::new(ClipboardManager::new());
        let monitor = PcClipboardMonitor::new(manager);
        assert!(monitor.is_ok());
        let monitor = monitor.unwrap();
        assert!(!monitor.is_running());
    }

    #[cfg(not(feature = "clipboard"))]
    #[test]
    fn test_clipboard_monitor_start_stop() {
        let manager = Arc::new(ClipboardManager::new());
        let mut monitor = PcClipboardMonitor::new(manager).unwrap();
        monitor.start().unwrap();
        assert!(monitor.is_running());
        monitor.stop().unwrap();
        assert!(!monitor.is_running());
    }

    #[cfg(not(feature = "clipboard"))]
    #[test]
    fn test_set_system_clipboard_stub() {
        let manager = Arc::new(ClipboardManager::new());
        let mut monitor = PcClipboardMonitor::new(manager).unwrap();
        monitor.set_system_clipboard("test").unwrap();
        assert_eq!(monitor.last_content, Some("test".to_string()));
    }

    #[cfg(not(feature = "clipboard"))]
    #[tokio::test]
    async fn test_check_and_sync_not_running() {
        let manager = Arc::new(ClipboardManager::new());
        let mut monitor = PcClipboardMonitor::new(manager).unwrap();
        let result = monitor.check_and_sync().await.unwrap();
        assert!(result.is_none());
    }

    #[cfg(not(feature = "clipboard"))]
    #[tokio::test]
    async fn test_sync_to_system_empty() {
        let manager = Arc::new(ClipboardManager::new());
        let mut monitor = PcClipboardMonitor::new(manager).unwrap();
        monitor.sync_to_system().await.unwrap();
    }
}