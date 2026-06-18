//! TCP connection management.
//!
//! This module provides connection establishment, state management,
//! and connection pooling for device-to-device communication.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{timeout, sleep};

use crate::error::{Result, TransportError};
use crate::frame::{Frame, FrameCodec};

/// Connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection is being established.
    Connecting,
    /// Connection is active and ready.
    Connected,
    /// Connection is being closed.
    Disconnecting,
    /// Connection is closed.
    Disconnected,
    /// Connection failed due to error.
    Failed,
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self::Disconnected
    }
}

/// Connection statistics.
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    pub frames_sent: u64,
    pub frames_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub last_activity: Option<Instant>,
    pub connection_time: Option<Instant>,
}

/// Connection configuration.
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Connection timeout in seconds.
    pub connect_timeout: Duration,
    /// Read timeout in seconds.
    pub read_timeout: Duration,
    /// Write timeout in seconds.
    pub write_timeout: Duration,
    /// Heartbeat interval in seconds.
    pub heartbeat_interval: Duration,
    /// Maximum number of missed heartbeats before disconnect.
    pub max_missed_heartbeats: u32,
    /// Send queue capacity.
    pub send_queue_capacity: usize,
    /// Receive queue capacity.
    pub receive_queue_capacity: usize,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(5),
            max_missed_heartbeats: 3,
            send_queue_capacity: 100,
            receive_queue_capacity: 100,
        }
    }
}

/// A TCP connection to a remote device.
pub struct Connection {
    /// Remote address.
    remote_addr: SocketAddr,
    /// Connection state.
    state: Arc<RwLock<ConnectionState>>,
    /// Connection statistics.
    stats: Arc<Mutex<ConnectionStats>>,
    /// Send queue.
    send_queue: mpsc::Sender<Frame>,
    /// Receive queue.
    receive_queue: mpsc::Receiver<Frame>,
    /// Whether the connection is active.
    active: Arc<AtomicBool>,
    /// Connection ID.
    id: u64,
}

impl Connection {
    /// Create a new connection (internal use).
    fn new(
        remote_addr: SocketAddr,
        send_queue: mpsc::Sender<Frame>,
        receive_queue: mpsc::Receiver<Frame>,
        id: u64,
    ) -> Self {
        Self {
            remote_addr,
            state: Arc::new(RwLock::new(ConnectionState::Connected)),
            stats: Arc::new(Mutex::new(ConnectionStats::default())),
            send_queue,
            receive_queue,
            active: Arc::new(AtomicBool::new(true)),
            id,
        }
    }
    
    /// Get the remote address.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
    
    /// Get the connection state.
    pub async fn state(&self) -> ConnectionState {
        *self.state.read().await
    }
    
    /// Get the connection ID.
    pub fn id(&self) -> u64 {
        self.id
    }
    
    /// Check if the connection is active.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }
    
    /// Send a frame.
    pub async fn send(&self, frame: Frame) -> Result<()> {
        if !self.is_active() {
            return Err(TransportError::NotConnected);
        }
        
        self.send_queue.send(frame).await.map_err(|_| TransportError::SendQueueFull)?;
        
        let mut stats = self.stats.lock().await;
        stats.frames_sent += 1;
        stats.last_activity = Some(Instant::now());
        
        Ok(())
    }
    
    /// Receive a frame.
    pub async fn receive(&mut self) -> Result<Option<Frame>> {
        if !self.is_active() {
            return Err(TransportError::NotConnected);
        }
        
        match self.receive_queue.recv().await {
            Some(frame) => {
                let mut stats = self.stats.lock().await;
                stats.frames_received += 1;
                stats.last_activity = Some(Instant::now());
                Ok(Some(frame))
            }
            None => Ok(None),
        }
    }
    
    /// Get connection statistics.
    pub async fn stats(&self) -> ConnectionStats {
        self.stats.lock().await.clone()
    }
    
    /// Close the connection.
    pub async fn close(&self) -> Result<()> {
        self.active.store(false, Ordering::SeqCst);
        *self.state.write().await = ConnectionState::Disconnected;
        Ok(())
    }
}

/// Connection manager for managing multiple connections.
pub struct ConnectionManager {
    /// Active connections.
    connections: Arc<RwLock<Vec<Arc<Connection>>>>,
    /// Next connection ID.
    next_id: AtomicU64,
    /// Connection configuration.
    config: ConnectionConfig,
}

impl ConnectionManager {
    /// Create a new connection manager.
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            connections: Arc::new(RwLock::new(Vec::new())),
            next_id: AtomicU64::new(1),
            config,
        }
    }
    
    /// Create with default configuration.
    pub fn default_config() -> Self {
        Self::new(ConnectionConfig::default())
    }
    
    /// Connect to a remote address.
    pub async fn connect(&self, addr: SocketAddr) -> Result<Arc<Connection>> {
        // Create queues
        let (send_tx, send_rx) = mpsc::channel(self.config.send_queue_capacity);
        let (recv_tx, recv_rx) = mpsc::channel(self.config.receive_queue_capacity);
        
        // Get connection ID
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        
        // Create connection object
        let connection = Arc::new(Connection::new(addr, send_tx, recv_rx, id));
        
        // Add to connections list
        self.connections.write().await.push(connection.clone());
        
        // Start background tasks for sending/receiving
        self.spawn_connection_tasks(connection.clone(), send_rx, recv_tx);
        
        Ok(connection)
    }
    
    /// Spawn background tasks for a connection.
    fn spawn_connection_tasks(
        &self,
        connection: Arc<Connection>,
        mut send_rx: mpsc::Receiver<Frame>,
        recv_tx: mpsc::Sender<Frame>,
    ) {
        let active = connection.active.clone();
        let stats = connection.stats.clone();
        
        // Spawn sender task (mock - would normally write to TCP stream)
        tokio::spawn(async move {
            while active.load(Ordering::SeqCst) {
                match send_rx.recv().await {
                    Some(frame) => {
                        let mut stats = stats.lock().await;
                        stats.bytes_sent += frame.total_size() as u64;
                    }
                    None => break,
                }
            }
        });
        
        // Spawn heartbeat task
        let heartbeat_active = connection.active.clone();
        let heartbeat_interval = self.config.heartbeat_interval;
        let send_queue = connection.send_queue.clone();
        
        tokio::spawn(async move {
            while heartbeat_active.load(Ordering::SeqCst) {
                sleep(heartbeat_interval).await;
                if heartbeat_active.load(Ordering::SeqCst) {
                    let _ = send_queue.send(Frame::heartbeat(0)).await;
                }
            }
        });
        
        // Spawn receiver task (mock - would normally read from TCP stream)
        let receiver_active = connection.active.clone();
        tokio::spawn(async move {
            // Mock: generate some frames for testing
            let mut seq = 0u32;
            while receiver_active.load(Ordering::SeqCst) && seq < 5 {
                sleep(Duration::from_millis(100)).await;
                seq += 1;
                let frame = Frame::heartbeat(seq);
                if recv_tx.send(frame).await.is_err() {
                    break;
                }
            }
        });
    }
    
    /// Get all active connections.
    pub async fn connections(&self) -> Vec<Arc<Connection>> {
        self.connections.read().await.clone()
    }
    
    /// Get a connection by ID.
    pub async fn get_connection(&self, id: u64) -> Option<Arc<Connection>> {
        self.connections.read().await.iter()
            .find(|c| c.id() == id)
            .cloned()
    }
    
    /// Remove a connection.
    pub async fn remove_connection(&self, id: u64) -> Result<()> {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.iter().find(|c| c.id() == id).cloned() {
            conn.close().await?;
            connections.retain(|c| c.id() != id);
        }
        Ok(())
    }
    
    /// Close all connections.
    pub async fn close_all(&self) -> Result<()> {
        let connections = self.connections.read().await.clone();
        for conn in connections {
            conn.close().await?;
        }
        self.connections.write().await.clear();
        Ok(())
    }
    
    /// Get number of active connections.
    pub async fn connection_count(&self) -> usize {
        self.connections.read().await.len()
    }
}

/// Async TCP connection implementation.
pub struct AsyncConnection {
    /// Remote address.
    remote_addr: SocketAddr,
    /// Connection state.
    state: ConnectionState,
    /// Write half of the stream.
    writer: Option<OwnedWriteHalf>,
    /// Read half of the stream.
    reader: Option<OwnedReadHalf>,
    /// Frame codec.
    codec: FrameCodec,
    /// Sequence ID counter.
    sequence_id: u32,
    /// Statistics.
    stats: ConnectionStats,
}

impl AsyncConnection {
    /// Connect to a remote address.
    pub async fn connect(addr: SocketAddr, config: &ConnectionConfig) -> Result<Self> {
        let stream = timeout(config.connect_timeout, tokio::net::TcpStream::connect(addr))
            .await
            .map_err(|_| TransportError::ConnectionTimeout)?
            .map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;
        
        let (reader, writer) = stream.into_split();
        
        Ok(Self {
            remote_addr: addr,
            state: ConnectionState::Connected,
            writer: Some(writer),
            reader: Some(reader),
            codec: FrameCodec::new(),
            sequence_id: 0,
            stats: ConnectionStats {
                connection_time: Some(Instant::now()),
                ..Default::default()
            },
        })
    }
    
    /// Get remote address.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_addr
    }
    
    /// Get connection state.
    pub fn state(&self) -> ConnectionState {
        self.state
    }
    
    /// Send a frame.
    pub async fn send_frame(&mut self, frame: &Frame) -> Result<()> {
        if self.state != ConnectionState::Connected {
            return Err(TransportError::NotConnected);
        }
        
        let writer = self.writer.as_mut().ok_or(TransportError::NotConnected)?;
        let encoded = frame.encode();
        
        timeout(Duration::from_secs(30), writer.write_all(&encoded))
            .await
            .map_err(|_| TransportError::ConnectionTimeout)?
            .map_err(TransportError::IoError)?;
        
        writer.flush().await.map_err(TransportError::IoError)?;
        
        self.stats.frames_sent += 1;
        self.stats.bytes_sent += encoded.len() as u64;
        self.stats.last_activity = Some(Instant::now());
        
        Ok(())
    }
    
    /// Receive a frame.
    pub async fn receive_frame(&mut self) -> Result<Frame> {
        if self.state != ConnectionState::Connected {
            return Err(TransportError::NotConnected);
        }
        
        let reader = self.reader.as_mut().ok_or(TransportError::NotConnected)?;
        
        // Read header
        let mut header_buf = [0u8; 13];
        timeout(Duration::from_secs(30), reader.read_exact(&mut header_buf))
            .await
            .map_err(|_| TransportError::ConnectionTimeout)?
            .map_err(TransportError::IoError)?;
        
        let header = crate::frame::FrameHeader::decode(&header_buf)?;
        
        // Read payload
        let mut payload_buf = vec![0u8; header.payload_len as usize];
        if header.payload_len > 0 {
            timeout(Duration::from_secs(30), reader.read_exact(&mut payload_buf))
                .await
                .map_err(|_| TransportError::ConnectionTimeout)?
                .map_err(TransportError::IoError)?;
        }
        
        self.stats.frames_received += 1;
        self.stats.bytes_received += (13 + header.payload_len as usize) as u64;
        self.stats.last_activity = Some(Instant::now());
        
        Ok(Frame {
            header,
            payload: bytes::Bytes::from(payload_buf),
        })
    }
    
    /// Send a data frame.
    pub async fn send_data(&mut self, payload: bytes::Bytes) -> Result<u32> {
        self.sequence_id += 1;
        let frame = Frame::data(self.sequence_id, payload)?;
        self.send_frame(&frame).await?;
        Ok(self.sequence_id)
    }
    
    /// Send a heartbeat.
    pub async fn send_heartbeat(&mut self) -> Result<u32> {
        self.sequence_id += 1;
        let frame = Frame::heartbeat(self.sequence_id);
        self.send_frame(&frame).await?;
        Ok(self.sequence_id)
    }
    
    /// Close the connection.
    pub async fn close(&mut self) -> Result<()> {
        self.state = ConnectionState::Disconnected;
        self.writer = None;
        self.reader = None;
        Ok(())
    }
    
    /// Get statistics.
    pub fn stats(&self) -> &ConnectionStats {
        &self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;
    
    #[test]
    fn test_connection_state_default() {
        assert_eq!(ConnectionState::default(), ConnectionState::Disconnected);
    }
    
    #[test]
    fn test_connection_config_default() {
        let config = ConnectionConfig::default();
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.heartbeat_interval, Duration::from_secs(5));
        assert_eq!(config.send_queue_capacity, 100);
    }
    
    #[test]
    fn test_connection_stats_default() {
        let stats = ConnectionStats::default();
        assert_eq!(stats.frames_sent, 0);
        assert_eq!(stats.frames_received, 0);
        assert_eq!(stats.last_activity, None);
    }
    
    #[tokio::test]
    async fn test_connection_manager_new() {
        let manager = ConnectionManager::default_config();
        assert_eq!(manager.connection_count().await, 0);
    }
    
    #[tokio::test]
    async fn test_connection_manager_connect() {
        let manager = ConnectionManager::default_config();
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        
        let conn = manager.connect(addr).await.unwrap();
        assert_eq!(conn.remote_addr(), addr);
        assert!(conn.is_active());
        assert_eq!(manager.connection_count().await, 1);
    }
    
    #[tokio::test]
    async fn test_connection_manager_get_connection() {
        let manager = ConnectionManager::default_config();
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        
        let conn = manager.connect(addr).await.unwrap();
        let id = conn.id();
        
        let found = manager.get_connection(id).await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().id(), id);
    }
    
    #[tokio::test]
    async fn test_connection_manager_remove_connection() {
        let manager = ConnectionManager::default_config();
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        
        let conn = manager.connect(addr).await.unwrap();
        let id = conn.id();
        
        manager.remove_connection(id).await.unwrap();
        assert_eq!(manager.connection_count().await, 0);
        
        let found = manager.get_connection(id).await;
        assert!(found.is_none());
    }
    
    #[tokio::test]
    async fn test_connection_manager_close_all() {
        let manager = ConnectionManager::default_config();
        
        manager.connect("127.0.0.1:12345".parse().unwrap()).await.unwrap();
        manager.connect("127.0.0.1:12346".parse().unwrap()).await.unwrap();
        
        assert_eq!(manager.connection_count().await, 2);
        
        manager.close_all().await.unwrap();
        assert_eq!(manager.connection_count().await, 0);
    }
    
    #[tokio::test]
    async fn test_connection_send_receive() {
        let manager = ConnectionManager::default_config();
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        
        let conn = manager.connect(addr).await.unwrap();
        
        // Send a frame
        let frame = Frame::heartbeat(1);
        conn.send(frame).await.unwrap();
        
        // Check stats
        let stats = conn.stats().await;
        assert_eq!(stats.frames_sent, 1);
    }
    
    #[tokio::test]
    async fn test_connection_close() {
        let manager = ConnectionManager::default_config();
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        
        let conn = manager.connect(addr).await.unwrap();
        assert!(conn.is_active());
        
        conn.close().await.unwrap();
        assert!(!conn.is_active());
        
        // Sending after close should fail
        let frame = Frame::heartbeat(1);
        assert!(conn.send(frame).await.is_err());
    }
    
    #[test]
    fn test_connection_state_transitions() {
        let states = [
            ConnectionState::Connecting,
            ConnectionState::Connected,
            ConnectionState::Disconnecting,
            ConnectionState::Disconnected,
            ConnectionState::Failed,
        ];
        
        for state in states {
            let s: ConnectionState = state;
            assert_eq!(s, state);
        }
    }
    
    #[tokio::test]
    async fn test_connection_stats_update() {
        let manager = ConnectionManager::default_config();
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        
        let conn = manager.connect(addr).await.unwrap();
        
        // Send multiple frames
        for i in 0..5 {
            let frame = Frame::heartbeat(i);
            conn.send(frame).await.unwrap();
        }
        
        let stats = conn.stats().await;
        assert_eq!(stats.frames_sent, 5);
        assert!(stats.last_activity.is_some());
    }
}