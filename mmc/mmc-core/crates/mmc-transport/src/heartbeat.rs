//! Heartbeat and keepalive mechanism.
//!
//! This module provides heartbeat monitoring for connection health
//! and automatic disconnection when heartbeats are missed.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::interval;

use crate::error::{Result, TransportError};
use crate::frame::Frame;

/// Heartbeat configuration.
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    /// Interval between heartbeat frames.
    pub interval: Duration,
    /// Maximum number of missed heartbeats before declaring timeout.
    pub max_missed: u32,
    /// Timeout for waiting for heartbeat response.
    pub response_timeout: Duration,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(5),
            max_missed: 3,
            response_timeout: Duration::from_secs(10),
        }
    }
}

/// Heartbeat state.
#[derive(Debug, Clone)]
pub struct HeartbeatState {
    /// Number of heartbeats sent.
    pub sent_count: u32,
    /// Number of heartbeats received.
    pub received_count: u32,
    /// Number of missed heartbeats.
    pub missed_count: u32,
    /// Last heartbeat sent time.
    pub last_sent: Option<Instant>,
    /// Last heartbeat received time.
    pub last_received: Option<Instant>,
    /// Whether heartbeat is active.
    pub is_active: bool,
}

impl Default for HeartbeatState {
    fn default() -> Self {
        Self {
            sent_count: 0,
            received_count: 0,
            missed_count: 0,
            last_sent: None,
            last_received: None,
            is_active: false,
        }
    }
}

/// Heartbeat monitor for tracking connection health.
pub struct HeartbeatMonitor {
    /// Configuration.
    config: HeartbeatConfig,
    /// Current state.
    state: Arc<RwLock<HeartbeatState>>,
    /// Whether monitoring is running.
    running: AtomicBool,
    /// Last sequence ID received.
    last_sequence_id: AtomicU32,
}

impl HeartbeatMonitor {
    /// Create a new heartbeat monitor.
    pub fn new(config: HeartbeatConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(HeartbeatState::default())),
            running: AtomicBool::new(false),
            last_sequence_id: AtomicU32::new(0),
        }
    }
    
    /// Create with default configuration.
    pub fn default_config() -> Self {
        Self::new(HeartbeatConfig::default())
    }
    
    /// Start heartbeat monitoring.
    pub async fn start(&self) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }
        
        self.running.store(true, Ordering::SeqCst);
        self.state.write().await.is_active = true;
        
        Ok(())
    }
    
    /// Stop heartbeat monitoring.
    pub async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        self.state.write().await.is_active = false;
        Ok(())
    }
    
    /// Check if monitoring is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
    
    /// Record a heartbeat sent.
    pub async fn record_sent(&self) {
        let mut state = self.state.write().await;
        state.sent_count += 1;
        state.last_sent = Some(Instant::now());
    }
    
    /// Record a heartbeat received.
    pub async fn record_received(&self, sequence_id: u32) {
        let mut state = self.state.write().await;
        state.received_count += 1;
        state.last_received = Some(Instant::now());
        state.missed_count = 0; // Reset missed count on receive
        self.last_sequence_id.store(sequence_id, Ordering::SeqCst);
    }
    
    /// Record a missed heartbeat.
    pub async fn record_missed(&self) {
        let mut state = self.state.write().await;
        state.missed_count += 1;
    }
    
    /// Check if connection is healthy (not too many missed heartbeats).
    pub async fn is_healthy(&self) -> bool {
        let state = self.state.read().await;
        state.missed_count < self.config.max_missed
    }
    
    /// Check if heartbeat timeout has occurred.
    pub async fn is_timeout(&self) -> bool {
        let state = self.state.read().await;
        state.missed_count >= self.config.max_missed
    }
    
    /// Get current state.
    pub async fn state(&self) -> HeartbeatState {
        self.state.read().await.clone()
    }
    
    /// Get configuration.
    pub fn config(&self) -> &HeartbeatConfig {
        &self.config
    }
    
    /// Get last received sequence ID.
    pub fn last_sequence_id(&self) -> u32 {
        self.last_sequence_id.load(Ordering::SeqCst)
    }
    
    /// Generate a heartbeat frame.
    pub fn generate_heartbeat(&self, sequence_id: u32) -> Frame {
        Frame::heartbeat(sequence_id)
    }
    
    /// Check if a frame is a heartbeat.
    pub fn is_heartbeat(frame: &Frame) -> bool {
        frame.is_heartbeat()
    }
    
    /// Get time since last heartbeat sent.
    pub async fn time_since_sent(&self) -> Option<Duration> {
        self.state.read().await.last_sent.map(|t| t.elapsed())
    }
    
    /// Get time since last heartbeat received.
    pub async fn time_since_received(&self) -> Option<Duration> {
        self.state.read().await.last_received.map(|t| t.elapsed())
    }
    
    /// Check if we should send a heartbeat now.
    pub async fn should_send_heartbeat(&self) -> bool {
        if !self.running.load(Ordering::SeqCst) {
            return false;
        }
        
        let state = self.state.read().await;
        match state.last_sent {
            Some(last) => last.elapsed() >= self.config.interval,
            None => true, // Never sent, should send now
        }
    }
    
    /// Check if we should consider a heartbeat missed.
    pub async fn should_check_missed(&self) -> bool {
        let state = self.state.read().await;
        match state.last_received {
            Some(last) => last.elapsed() >= self.config.response_timeout,
            None => false, // No baseline yet
        }
    }
}

/// Heartbeat sender task.
pub struct HeartbeatSender {
    monitor: Arc<HeartbeatMonitor>,
    sequence_id: AtomicU32,
}

impl HeartbeatSender {
    /// Create a new heartbeat sender.
    pub fn new(monitor: Arc<HeartbeatMonitor>) -> Self {
        Self {
            monitor,
            sequence_id: AtomicU32::new(0),
        }
    }
    
    /// Run the heartbeat sender loop.
    pub async fn run<F>(&self, send_fn: F) -> Result<()> 
    where 
        F: Fn(Frame) -> Result<()> + Send + Sync
    {
        self.monitor.start().await?;
        
        let mut ticker = interval(self.monitor.config().interval);
        
        while self.monitor.is_running() {
            ticker.tick().await;
            
            if !self.monitor.is_running() {
                break;
            }
            
            // Check if connection is healthy
            if !self.monitor.is_healthy().await {
                return Err(TransportError::HeartbeatTimeout);
            }
            
            // Send heartbeat
            let seq = self.sequence_id.fetch_add(1, Ordering::SeqCst) + 1;
            let frame = self.monitor.generate_heartbeat(seq);
            
            if let Err(e) = send_fn(frame) {
                tracing::warn!("Failed to send heartbeat: {}", e);
                self.monitor.record_missed().await;
            } else {
                self.monitor.record_sent().await;
            }
        }
        
        self.monitor.stop().await?;
        Ok(())
    }
    
    /// Stop the sender.
    pub async fn stop(&self) -> Result<()> {
        self.monitor.stop().await?;
        Ok(())
    }
}

/// Heartbeat receiver task.
pub struct HeartbeatReceiver {
    monitor: Arc<HeartbeatMonitor>,
}

impl HeartbeatReceiver {
    /// Create a new heartbeat receiver.
    pub fn new(monitor: Arc<HeartbeatMonitor>) -> Self {
        Self { monitor }
    }
    
    /// Handle a received frame.
    pub async fn handle_frame(&self, frame: &Frame) -> bool {
        if HeartbeatMonitor::is_heartbeat(frame) {
            self.monitor.record_received(frame.header.sequence_id).await;
            return true;
        }
        false
    }
    
    /// Get the monitor.
    pub fn monitor(&self) -> &HeartbeatMonitor {
        &*self.monitor
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_heartbeat_config_default() {
        let config = HeartbeatConfig::default();
        assert_eq!(config.interval, Duration::from_secs(5));
        assert_eq!(config.max_missed, 3);
        assert_eq!(config.response_timeout, Duration::from_secs(10));
    }
    
    #[test]
    fn test_heartbeat_state_default() {
        let state = HeartbeatState::default();
        assert_eq!(state.sent_count, 0);
        assert_eq!(state.received_count, 0);
        assert_eq!(state.missed_count, 0);
        assert!(!state.is_active);
    }
    
    #[tokio::test]
    async fn test_heartbeat_monitor_new() {
        let monitor = HeartbeatMonitor::default_config();
        assert!(!monitor.is_running());
        assert_eq!(monitor.last_sequence_id(), 0);
    }
    
    #[tokio::test]
    async fn test_heartbeat_monitor_start_stop() {
        let monitor = HeartbeatMonitor::default_config();
        
        monitor.start().await.unwrap();
        assert!(monitor.is_running());
        
        let state = monitor.state().await;
        assert!(state.is_active);
        
        monitor.stop().await.unwrap();
        assert!(!monitor.is_running());
    }
    
    #[tokio::test]
    async fn test_heartbeat_monitor_record_sent() {
        let monitor = HeartbeatMonitor::default_config();
        monitor.start().await.unwrap();
        
        monitor.record_sent().await;
        
        let state = monitor.state().await;
        assert_eq!(state.sent_count, 1);
        assert!(state.last_sent.is_some());
    }
    
    #[tokio::test]
    async fn test_heartbeat_monitor_record_received() {
        let monitor = HeartbeatMonitor::default_config();
        monitor.start().await.unwrap();
        
        monitor.record_received(42).await;
        
        let state = monitor.state().await;
        assert_eq!(state.received_count, 1);
        assert!(state.last_received.is_some());
        assert_eq!(monitor.last_sequence_id(), 42);
    }
    
    #[tokio::test]
    async fn test_heartbeat_monitor_record_missed() {
        let monitor = HeartbeatMonitor::default_config();
        monitor.start().await.unwrap();
        
        monitor.record_missed().await;
        monitor.record_missed().await;
        
        let state = monitor.state().await;
        assert_eq!(state.missed_count, 2);
    }
    
    #[tokio::test]
    async fn test_heartbeat_monitor_is_healthy() {
        let monitor = HeartbeatMonitor::default_config();
        monitor.start().await.unwrap();
        
        assert!(monitor.is_healthy().await);
        
        // Record missed heartbeats up to limit
        for _ in 0..monitor.config().max_missed {
            monitor.record_missed().await;
        }
        
        assert!(!monitor.is_healthy().await);
        assert!(monitor.is_timeout().await);
    }
    
    #[tokio::test]
    async fn test_heartbeat_monitor_receive_reset_missed() {
        let monitor = HeartbeatMonitor::default_config();
        monitor.start().await.unwrap();
        
        // Record some missed
        monitor.record_missed().await;
        monitor.record_missed().await;
        
        let state = monitor.state().await;
        assert_eq!(state.missed_count, 2);
        
        // Receive heartbeat - should reset missed count
        monitor.record_received(1).await;
        
        let state = monitor.state().await;
        assert_eq!(state.missed_count, 0);
    }
    
    #[test]
    fn test_heartbeat_monitor_generate_heartbeat() {
        let monitor = HeartbeatMonitor::default_config();
        let frame = monitor.generate_heartbeat(100);
        
        assert!(frame.is_heartbeat());
        assert_eq!(frame.header.sequence_id, 100);
    }
    
    #[test]
    fn test_heartbeat_monitor_is_heartbeat() {
        let heartbeat = Frame::heartbeat(1);
        let data = Frame::data(1, bytes::Bytes::from("test")).unwrap();
        
        assert!(HeartbeatMonitor::is_heartbeat(&heartbeat));
        assert!(!HeartbeatMonitor::is_heartbeat(&data));
    }
    
    #[tokio::test]
    async fn test_heartbeat_monitor_should_send() {
        let monitor = HeartbeatMonitor::default_config();
        monitor.start().await.unwrap();
        
        // Should send immediately after start
        assert!(monitor.should_send_heartbeat().await);
        
        // Record sent
        monitor.record_sent().await;
        
        // Should not send right after
        assert!(!monitor.should_send_heartbeat().await);
    }
    
    #[tokio::test]
    async fn test_heartbeat_sender_new() {
        let monitor = Arc::new(HeartbeatMonitor::default_config());
        let sender = HeartbeatSender::new(monitor.clone());
        
        assert_eq!(sender.sequence_id.load(Ordering::SeqCst), 0);
    }
    
    #[tokio::test]
    async fn test_heartbeat_receiver_new() {
        let monitor = Arc::new(HeartbeatMonitor::default_config());
        let receiver = HeartbeatReceiver::new(monitor.clone());
        
        assert!(!receiver.monitor().is_running());
    }
    
    #[tokio::test]
    async fn test_heartbeat_receiver_handle_frame() {
        let monitor = Arc::new(HeartbeatMonitor::default_config());
        monitor.start().await.unwrap();
        
        let receiver = HeartbeatReceiver::new(monitor.clone());
        
        // Handle heartbeat frame
        let heartbeat = Frame::heartbeat(42);
        let handled = receiver.handle_frame(&heartbeat).await;
        assert!(handled);
        assert_eq!(monitor.last_sequence_id(), 42);
        
        // Handle data frame
        let data = Frame::data(1, bytes::Bytes::from("test")).unwrap();
        let handled = receiver.handle_frame(&data).await;
        assert!(!handled);
    }
    
    #[tokio::test]
    async fn test_time_since_sent_received() {
        let monitor = HeartbeatMonitor::default_config();
        monitor.start().await.unwrap();
        
        // Initially no time
        assert!(monitor.time_since_sent().await.is_none());
        assert!(monitor.time_since_received().await.is_none());
        
        // Record sent
        monitor.record_sent().await;
        assert!(monitor.time_since_sent().await.is_some());
        
        // Record received
        monitor.record_received(1).await;
        assert!(monitor.time_since_received().await.is_some());
    }
}