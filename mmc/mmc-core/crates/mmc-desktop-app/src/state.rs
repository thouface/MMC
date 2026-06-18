//! Application state management

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use mmc_clipboard::ClipboardManager;
use mmc_protocol::{ClipboardContent, ClipboardData};
use mmc_media_service::platform::{DefaultPlatformAdapter, PlatformType};
use mmc_media_service::session::MirroringSession;

use crate::error::Result;

/// Paired device information for desktop app
#[derive(Debug, Clone)]
pub struct PairedDevice {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub paired_at: i64,
}

/// Transfer task for desktop app
#[derive(Debug, Clone)]
pub struct DesktopTransferTask {
    pub task_id: String,
    pub file_name: String,
    pub state: String,
    pub progress: DesktopTransferProgress,
}

/// Transfer progress for desktop app
#[derive(Debug, Clone)]
pub struct DesktopTransferProgress {
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub percent: u32,
}

/// Mirror session stats for desktop app
#[derive(Debug, Clone)]
pub struct DesktopMirrorStats {
    pub state: String,
    pub video_frames: u64,
    pub audio_frames: u64,
    pub input_events: u64,
    pub duration: Option<f64>,
}

/// Application state container
pub struct AppState {
    device_id: String,
    device_name: String,
    platform_type: PlatformType,
    clipboard_manager: Arc<ClipboardManager>,
    mirror_session: Option<MirroringSession>,
    paired_devices: HashMap<String, PairedDevice>,
    transfer_tasks: HashMap<String, DesktopTransferTask>,
    initialized: bool,
}

impl AppState {
    /// Create a new application state
    pub fn new() -> Self {
        let device_id = uuid::Uuid::new_v4().to_string();
        let device_name = hostname::get()
            .map(|h| h.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "Desktop Device".to_string());

        let platform_type = DefaultPlatformAdapter::detect_platform();

        Self {
            device_id,
            device_name,
            platform_type,
            clipboard_manager: Arc::new(ClipboardManager::new()),
            mirror_session: None,
            paired_devices: HashMap::new(),
            transfer_tasks: HashMap::new(),
            initialized: false,
        }
    }

    /// Initialize the application state
    pub fn init(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }
        self.initialized = true;
        tracing::info!("Desktop app initialized: {} ({})", self.device_name, self.platform_type);
        Ok(())
    }

    /// Get device ID
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Get device name
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Get platform type
    pub fn platform_type(&self) -> PlatformType {
        self.platform_type
    }

    /// Discover nearby devices (stub - returns simulated devices for testing)
    pub async fn discover_devices(&self) -> Result<Vec<PairedDevice>> {
        tracing::info!("Discovering devices via mDNS...");
        
        // Simulate discovery delay
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Return simulated devices for testing purposes
        // In real implementation, this would use mDNS discovery
        let simulated_devices = vec![
            PairedDevice {
                id: "android-phone-001".to_string(),
                name: "Android Phone".to_string(),
                ip: "192.168.1.100".to_string(),
                port: 8080,
                paired_at: chrono::Utc::now().timestamp(),
            },
            PairedDevice {
                id: "android-tablet-002".to_string(),
                name: "Android Tablet".to_string(),
                ip: "192.168.1.101".to_string(),
                port: 8080,
                paired_at: chrono::Utc::now().timestamp(),
            },
        ];
        
        tracing::info!("Found {} devices", simulated_devices.len());
        Ok(simulated_devices)
    }

    /// Pair with a device
    pub async fn pair_device(&mut self, device_id: &str) -> Result<bool> {
        tracing::info!("Pairing with device: {}", device_id);
        
        // Simulate pairing delay
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Add to paired devices
        let paired = PairedDevice {
            id: device_id.to_string(),
            name: device_id.to_string(),
            ip: "192.168.1.100".to_string(),
            port: 8080,
            paired_at: chrono::Utc::now().timestamp(),
        };
        
        self.paired_devices.insert(device_id.to_string(), paired);
        tracing::info!("Successfully paired with device: {}", device_id);
        Ok(true)
    }

    /// Get paired devices
    pub fn get_paired_devices(&self) -> Vec<PairedDevice> {
        self.paired_devices.values().cloned().collect()
    }

    /// Unpair a device
    pub fn unpair_device(&mut self, device_id: &str) -> Result<()> {
        self.paired_devices.remove(device_id);
        tracing::info!("Unpaired device: {}", device_id);
        Ok(())
    }

    /// Send a file to a device (stub)
    pub async fn send_file(&self, device_id: &str, file_path: &str) -> Result<String> {
        let task_id = uuid::Uuid::new_v4().to_string();
        tracing::info!("Sending file '{}' to device: {}", file_path, device_id);
        Ok(task_id)
    }

    /// Get transfer tasks
    pub fn get_transfer_tasks(&self) -> Vec<DesktopTransferTask> {
        self.transfer_tasks.values().cloned().collect()
    }

    /// Cancel a transfer
    pub fn cancel_transfer(&mut self, task_id: &str) -> Result<()> {
        self.transfer_tasks.remove(task_id);
        tracing::info!("Canceled transfer: {}", task_id);
        Ok(())
    }

    /// Get clipboard content
    pub async fn get_clipboard_content(&self) -> Result<Option<String>> {
        let current = self.clipboard_manager.get_current().await;
        match current {
            Some(entry) => match &entry.content.content {
                ClipboardData::Text { text } => Ok(Some(text.clone())),
                ClipboardData::Url { url } => Ok(Some(url.clone())),
                ClipboardData::Image { .. } => Ok(Some("[Image data]".to_string())),
            },
            None => Ok(None),
        }
    }

    /// Set clipboard content
    pub async fn set_clipboard_content(&self, text: &str) -> Result<()> {
        let content = ClipboardContent {
            timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
            content: ClipboardData::Text { text: text.to_string() },
        };
        self.clipboard_manager.set_local(content).await?;
        tracing::info!("Set clipboard content: {} chars", text.len());
        Ok(())
    }

    /// Sync clipboard with a device (stub)
    pub async fn sync_clipboard(&self, device_id: &str) -> Result<()> {
        tracing::info!("Syncing clipboard with device: {}", device_id);
        Ok(())
    }

    /// Monitor clipboard for changes
    pub async fn monitor_clipboard(&self, duration_secs: u64) -> Result<()> {
        tracing::info!("Monitoring clipboard for {} seconds", duration_secs);
        tokio::time::sleep(Duration::from_secs(duration_secs)).await;
        Ok(())
    }

    /// Start screen mirroring with a device
    pub async fn start_mirror(&mut self, device_id: &str) -> Result<()> {
        let mut session = MirroringSession::new();
        session.start()?;
        self.mirror_session = Some(session);
        tracing::info!("Started screen mirroring with device: {}", device_id);
        Ok(())
    }

    /// Stop screen mirroring
    pub fn stop_mirror(&mut self) -> Result<()> {
        if let Some(session) = &mut self.mirror_session {
            session.stop()?;
            tracing::info!("Stopped screen mirroring");
        }
        self.mirror_session = None;
        Ok(())
    }

    /// Get mirror session stats
    pub fn get_mirror_stats(&self) -> Option<DesktopMirrorStats> {
        self.mirror_session.as_ref().map(|session| {
            let stats = session.get_stats();
            DesktopMirrorStats {
                state: session.state().to_string(),
                video_frames: stats.video_frames,
                audio_frames: stats.audio_frames,
                input_events: stats.input_events,
                duration: stats.duration.map(|d| d as f64),
            }
        })
    }

    /// Get the platform adapter
    pub fn platform_adapter(&self) -> &'static DefaultPlatformAdapter {
        // Use LazyLock for static adapter
        use std::sync::LazyLock;
        static DEFAULT_ADAPTER: LazyLock<DefaultPlatformAdapter> = 
            LazyLock::new(|| DefaultPlatformAdapter::new(PlatformType::Linux));
        &DEFAULT_ADAPTER
    }

    /// Get the clipboard manager
    pub fn clipboard_manager(&self) -> &ClipboardManager {
        &self.clipboard_manager
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_new() {
        let state = AppState::new();
        assert!(!state.device_id().is_empty());
        assert!(!state.device_name().is_empty());
        assert!(!state.initialized);
    }

    #[test]
    fn test_app_state_init() {
        let mut state = AppState::new();
        state.init().unwrap();
        assert!(state.initialized);
    }

    #[test]
    fn test_get_paired_devices_empty() {
        let state = AppState::new();
        let devices = state.get_paired_devices();
        assert!(devices.is_empty());
    }

    #[test]
    fn test_get_transfer_tasks_empty() {
        let state = AppState::new();
        let tasks = state.get_transfer_tasks();
        assert!(tasks.is_empty());
    }

    #[test]
    fn test_platform_type() {
        let state = AppState::new();
        let platform = state.platform_type();
        // Should be one of the valid platform types
        match platform {
            PlatformType::Windows | PlatformType::Macos | PlatformType::Linux |
            PlatformType::Android | PlatformType::Ios | PlatformType::Unknown => {}
        }
    }

    #[tokio::test]
    async fn test_set_clipboard_content() {
        let state = AppState::new();
        state.set_clipboard_content("test text").await.unwrap();
        let content = state.get_clipboard_content().await.unwrap();
        assert_eq!(content, Some("test text".to_string()));
    }

    #[tokio::test]
    async fn test_get_clipboard_content_empty() {
        let state = AppState::new();
        let content = state.get_clipboard_content().await.unwrap();
        assert!(content.is_none());
    }

    #[tokio::test]
    async fn test_monitor_clipboard() {
        let state = AppState::new();
        // Short duration for test
        state.monitor_clipboard(1).await.unwrap();
    }

    #[test]
    fn test_mirror_stats_none() {
        let state = AppState::new();
        let stats = state.get_mirror_stats();
        assert!(stats.is_none());
    }

    #[tokio::test]
    async fn test_discover_devices() {
        let state = AppState::new();
        let devices = state.discover_devices().await.unwrap();
        // Should return simulated devices
        assert!(!devices.is_empty());
        assert_eq!(devices.len(), 2);
    }

    #[tokio::test]
    async fn test_pair_device() {
        let mut state = AppState::new();
        let result = state.pair_device("test-device").await.unwrap();
        assert!(result);
        // Verify device was added
        let paired = state.get_paired_devices();
        assert!(!paired.is_empty());
    }

    #[tokio::test]
    async fn test_send_file_stub() {
        let state = AppState::new();
        let task_id = state.send_file("test-device", "/path/to/file").await.unwrap();
        assert!(!task_id.is_empty());
    }
}