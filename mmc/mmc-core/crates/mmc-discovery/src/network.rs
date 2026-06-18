//! Network communication implementation for MMC devices

use super::{DeviceInfo, Error, Result};
use mmc_protocol::{Frame, FrameType, Heartbeat, Ping, Pong, read_frame, write_frame};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, mpsc};
use tokio::time;
use tracing::{debug, error, info, warn};

/// 网络连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Handshaking,
    Ready,
    Failed,
}

/// 网络连接结构
pub struct NetworkConnection {
    device_id: String,
    stream: Option<TcpStream>,
    state: ConnectionState,
    last_heartbeat_sent: Instant,
    last_heartbeat_received: Instant,
    heartbeat_interval: Duration,
    heartbeat_timeout: Duration,
}

impl NetworkConnection {
    pub fn new(device_id: &str) -> Self {
        Self {
            device_id: device_id.to_string(),
            stream: None,
            state: ConnectionState::Disconnected,
            last_heartbeat_sent: Instant::now(),
            last_heartbeat_received: Instant::now(),
            heartbeat_interval: Duration::from_secs(5),
            heartbeat_timeout: Duration::from_secs(15),
        }
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn state(&self) -> ConnectionState {
        self.state
    }

    pub fn is_heartbeat_timeout(&self) -> bool {
        self.last_heartbeat_received.elapsed() > self.heartbeat_timeout
    }

    pub async fn connect(&mut self, device: &DeviceInfo) -> Result<()> {
        self.state = ConnectionState::Connecting;
        
        let addr = format!("{}:{}", device.ip, device.port);
        let stream = TcpStream::connect(&addr).await
            .map_err(|e| Error::Network(e.to_string()))?;
        
        self.stream = Some(stream);
        self.state = ConnectionState::Connected;
        self.last_heartbeat_sent = Instant::now();
        self.last_heartbeat_received = Instant::now();
        
        Ok(())
    }

    pub async fn send_heartbeat(&mut self) -> Result<()> {
        let stream = self.stream.as_mut()
            .ok_or_else(|| Error::Network("Not connected".to_string()))?;

        let heartbeat = Heartbeat {
            timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
            device_id: self.device_id.clone(),
        };
        
        let frame = Frame::new(FrameType::Heartbeat, heartbeat.to_json().map_err(|e| Error::Protocol(e.to_string()))?);
        write_frame(stream, &frame).await.map_err(|e| Error::Protocol(e.to_string()))?;
        
        self.last_heartbeat_sent = Instant::now();
        debug!("Heartbeat sent to {}", self.device_id);
        
        Ok(())
    }

    pub async fn send_ping(&mut self) -> Result<Pong> {
        let stream = self.stream.as_mut()
            .ok_or_else(|| Error::Network("Not connected".to_string()))?;

        let ping = Ping {
            timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
        };
        
        let frame = Frame::new(FrameType::Ping, ping.to_json().map_err(|e| Error::Protocol(e.to_string()))?);
        write_frame(stream, &frame).await.map_err(|e| Error::Protocol(e.to_string()))?;

        let response = read_frame(stream).await.map_err(|e| Error::Protocol(e.to_string()))?
            .ok_or_else(|| Error::Protocol("Connection closed".to_string()))?;
        if response.frame_type == FrameType::Pong {
            let pong = Pong::from_json(&response.payload).map_err(|e| Error::Protocol(e.to_string()))?;
            Ok(pong)
        } else {
            Err(Error::Protocol("Expected Pong response".to_string()))
        }
    }

    pub async fn receive_frame(&mut self) -> Result<Frame> {
        let stream = self.stream.as_mut()
            .ok_or_else(|| Error::Network("Not connected".to_string()))?;

        let frame = read_frame(stream).await.map_err(|e| Error::Protocol(e.to_string()))?
            .ok_or_else(|| Error::Protocol("Connection closed".to_string()))?;
        
        if frame.frame_type == FrameType::Heartbeat {
            self.last_heartbeat_received = Instant::now();
            debug!("Heartbeat received from {}", self.device_id);
        }
        
        Ok(frame)
    }

    pub fn close(&mut self) {
        self.stream = None;
        self.state = ConnectionState::Disconnected;
    }
}

/// 网络管理器
pub struct NetworkManager {
    connections: Arc<RwLock<HashMap<String, NetworkConnection>>>,
    listener: Option<Arc<TcpListener>>,
    running: Arc<RwLock<bool>>,
    event_tx: mpsc::Sender<NetworkEvent>,
}

/// 网络事件
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    ConnectionEstablished(String),
    ConnectionLost(String),
    HeartbeatTimeout(String),
    MessageReceived(String, Frame),
}

impl NetworkManager {
    pub async fn new(event_tx: mpsc::Sender<NetworkEvent>) -> Result<Self> {
        Ok(Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            listener: None,
            running: Arc::new(RwLock::new(true)),
            event_tx,
        })
    }

    pub async fn start_listener(&mut self, port: u16) -> Result<()> {
        let addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(&addr).await
            .map_err(|e| Error::Network(e.to_string()))?;
        
        info!("Network listener started on {}", addr);
        
        let listener = Arc::new(listener);
        self.listener = Some(listener.clone());
        
        let running = self.running.clone();
        let connections = self.connections.clone();
        let event_tx = self.event_tx.clone();
        
        tokio::spawn(async move {
            while *running.read().await {
                match listener.accept().await {
                    Ok((stream, addr)) => {
                        info!("Incoming connection from {}", addr);
                        tokio::spawn(Self::handle_connection(stream, connections.clone(), event_tx.clone()));
                    }
                    Err(e) => {
                        error!("Accept error: {}", e);
                    }
                }
            }
        });
        
        Ok(())
    }

    async fn handle_connection(
        mut stream: TcpStream,
        connections: Arc<RwLock<HashMap<String, NetworkConnection>>>,
        event_tx: mpsc::Sender<NetworkEvent>,
    ) {
        loop {
            match read_frame(&mut stream).await {
                Ok(Some(frame)) => {
                    if frame.frame_type == FrameType::Heartbeat {
                        if let Ok(heartbeat) = Heartbeat::from_json(&frame.payload) {
                            let device_id = heartbeat.device_id.clone();
                            
                            let mut conns = connections.write().await;
                            if let Some(conn) = conns.get_mut(&device_id) {
                                conn.last_heartbeat_received = Instant::now();
                            }
                            
                            let _ = event_tx.send(NetworkEvent::MessageReceived(device_id, frame)).await;
                        }
                    }
                }
                Ok(None) => {
                    break;
                }
                Err(e) => {
                    warn!("Connection error: {}", e);
                    break;
                }
            }
        }
    }

    pub async fn connect_to_device(&self, device: &DeviceInfo) -> Result<()> {
        let mut connections = self.connections.write().await;
        
        if connections.contains_key(&device.id) {
            return Ok(());
        }
        
        let mut conn = NetworkConnection::new(&device.id);
        conn.connect(device).await?;
        
        connections.insert(device.id.clone(), conn);
        let _ = self.event_tx.send(NetworkEvent::ConnectionEstablished(device.id.clone())).await;
        
        Ok(())
    }

    pub async fn start_heartbeat_monitor(&self) {
        let running = self.running.clone();
        let connections = self.connections.clone();
        let event_tx = self.event_tx.clone();
        
        tokio::spawn(async move {
            while *running.read().await {
                time::sleep(Duration::from_secs(1)).await;
                
                let mut connections = connections.write().await;
                let mut timeout_devices: Vec<String> = Vec::new();
                
                for (device_id, conn) in connections.iter_mut() {
                    // 发送心跳（每5秒）
                    if conn.state == ConnectionState::Connected && 
                       conn.last_heartbeat_sent.elapsed() >= conn.heartbeat_interval {
                        if let Err(e) = conn.send_heartbeat().await {
                            warn!("Failed to send heartbeat to {}: {}", device_id, e);
                        }
                    }
                    
                    // 检测超时（15秒）
                    if conn.state == ConnectionState::Connected && conn.is_heartbeat_timeout() {
                        timeout_devices.push(device_id.clone());
                    }
                }
                
                for device_id in timeout_devices {
                    if let Some(mut conn) = connections.remove(&device_id) {
                        conn.close();
                        let _ = event_tx.send(NetworkEvent::HeartbeatTimeout(device_id.clone())).await;
                        let _ = event_tx.send(NetworkEvent::ConnectionLost(device_id)).await;
                    }
                }
            }
        });
    }

    pub async fn disconnect_device(&self, device_id: &str) {
        let mut connections = self.connections.write().await;
        if let Some(mut conn) = connections.remove(device_id) {
            conn.close();
            let _ = self.event_tx.send(NetworkEvent::ConnectionLost(device_id.to_string())).await;
        }
    }

    pub async fn get_connection_state(&self, device_id: &str) -> Option<ConnectionState> {
        let connections = self.connections.read().await;
        connections.get(device_id).map(|c| c.state())
    }

    pub async fn stop(&self) {
        *self.running.write().await = false;
        
        let mut connections = self.connections.write().await;
        for (_, conn) in connections.iter_mut() {
            conn.close();
        }
        connections.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_state_transitions() {
        let conn = NetworkConnection::new("test-device");
        assert_eq!(conn.state(), ConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn test_heartbeat_timeout() {
        let mut conn = NetworkConnection::new("test-device");
        conn.last_heartbeat_received = Instant::now() - Duration::from_secs(20);
        
        assert!(conn.is_heartbeat_timeout());
        
        conn.last_heartbeat_received = Instant::now();
        assert!(!conn.is_heartbeat_timeout());
    }

    #[tokio::test]
    async fn test_network_manager_connect() {
        let (tx, _rx) = mpsc::channel(10);
        let manager = NetworkManager::new(tx).await.unwrap();
        
        let device = DeviceInfo {
            id: "test-device".to_string(),
            name: "Test".to_string(),
            device_type: super::super::DeviceType::Pc,
            os_version: "1.0".to_string(),
            app_version: "1.0".to_string(),
            ip: "127.0.0.1".to_string(),
            port: 0,
            last_seen: chrono::Utc::now().timestamp(),
        };
        
        let result = manager.connect_to_device(&device).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_heartbeat_interval() {
        let conn = NetworkConnection::new("test-device");
        
        assert_eq!(conn.heartbeat_interval, Duration::from_secs(5));
        assert_eq!(conn.heartbeat_timeout, Duration::from_secs(15));
    }

    #[tokio::test]
    async fn test_network_event_types() {
        let events = vec![
            NetworkEvent::ConnectionEstablished("device1".to_string()),
            NetworkEvent::ConnectionLost("device1".to_string()),
            NetworkEvent::HeartbeatTimeout("device1".to_string()),
            NetworkEvent::MessageReceived("device1".to_string(), Frame::new(FrameType::Heartbeat, vec![])),
        ];
        
        assert!(matches!(events[0], NetworkEvent::ConnectionEstablished(_)));
        assert!(matches!(events[1], NetworkEvent::ConnectionLost(_)));
        assert!(matches!(events[2], NetworkEvent::HeartbeatTimeout(_)));
        assert!(matches!(events[3], NetworkEvent::MessageReceived(_, _)));
    }

    #[tokio::test]
    async fn test_connection_state_enum() {
        let states = vec![
            ConnectionState::Disconnected,
            ConnectionState::Connecting,
            ConnectionState::Connected,
            ConnectionState::Handshaking,
            ConnectionState::Ready,
            ConnectionState::Failed,
        ];
        
        assert_eq!(states.len(), 6);
    }
}
