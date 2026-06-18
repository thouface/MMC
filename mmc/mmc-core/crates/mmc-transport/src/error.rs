//! Error types for the transport layer.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Connection closed")]
    ConnectionClosed,
    
    #[error("Connection timeout")]
    ConnectionTimeout,
    
    #[error("Frame encoding failed: {0}")]
    FrameEncodeFailed(String),
    
    #[error("Frame decoding failed: {0}")]
    FrameDecodeFailed(String),
    
    #[error("Invalid frame header")]
    InvalidFrameHeader,
    
    #[error("Frame too large: {0} bytes (max: {1})")]
    FrameTooLarge(usize, usize),
    
    #[error("Incomplete frame: expected {0} bytes, got {1}")]
    IncompleteFrame(usize, usize),
    
    #[error("Heartbeat timeout")]
    HeartbeatTimeout,
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("TLS error: {0}")]
    TlsError(String),
    
    #[error("Not connected")]
    NotConnected,
    
    #[error("Already connected")]
    AlreadyConnected,
    
    #[error("Send queue full")]
    SendQueueFull,
    
    #[error("Receive queue full")]
    ReceiveQueueFull,
}

pub type Result<T> = std::result::Result<T, TransportError>;