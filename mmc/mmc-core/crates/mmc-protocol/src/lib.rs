//! Protocol types and frame definitions for MMC
//! Custom TCP frame protocol for device communication
//!
//! Supports both JSON and Protobuf serialization.

use bytes::{Buf, BufMut, BytesMut};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

pub mod error {
    use thiserror::Error;

    #[derive(Error, Debug)]
    pub enum ProtocolError {
        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),
        #[error("Unknown frame type: {0}")]
        UnknownFrameType(u16),
        #[error("Invalid frame: {0}")]
        InvalidFrame(String),
        #[error("Buffer too small: need {need}, got {got}")]
        BufferTooSmall { need: usize, got: usize },
        #[error("Serialization error: {0}")]
        Serialization(String),
    }

    pub type Result<T> = std::result::Result<T, ProtocolError>;
}
pub mod protobuf;

pub use error::ProtocolError;
pub use error::Result;

/// Frame types for the custom TCP protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum FrameType {
    // Pairing
    PairingRequest = 0x0101,
    PairingResponse = 0x0102,
    // File Transfer
    FileManifestRequest = 0x0201,
    FileManifestResponse = 0x0202,
    ChunkData = 0x0203,
    ChunkAck = 0x0204,
    TransferComplete = 0x0205,
    TransferError = 0x0206,
    // Remote Control
    TouchEvent = 0x0301,
    KeyEvent = 0x0302,
    ClipboardContent = 0x0303,
    // System
    Heartbeat = 0xFF01,
    Ping = 0xFF02,
    Pong = 0xFF03,
}

impl FrameType {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            0x0101 => Some(Self::PairingRequest),
            0x0102 => Some(Self::PairingResponse),
            0x0201 => Some(Self::FileManifestRequest),
            0x0202 => Some(Self::FileManifestResponse),
            0x0203 => Some(Self::ChunkData),
            0x0204 => Some(Self::ChunkAck),
            0x0205 => Some(Self::TransferComplete),
            0x0206 => Some(Self::TransferError),
            0x0301 => Some(Self::TouchEvent),
            0x0302 => Some(Self::KeyEvent),
            0x0303 => Some(Self::ClipboardContent),
            0xFF01 => Some(Self::Heartbeat),
            0xFF02 => Some(Self::Ping),
            0xFF03 => Some(Self::Pong),
            _ => None,
        }
    }
}

/// Custom TCP frame layout:
/// - 2 bytes: frame_type (u16, big-endian)
/// - 4 bytes: payload_length (u32, big-endian)
/// - N bytes: payload
#[derive(Debug, Clone)]
pub struct Frame {
    pub frame_type: FrameType,
    pub payload: Vec<u8>,
}

impl Frame {
    pub const HEADER_SIZE: usize = 6;

    pub fn new(frame_type: FrameType, payload: Vec<u8>) -> Self {
        Self { frame_type, payload }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = BytesMut::with_capacity(Self::HEADER_SIZE + self.payload.len());
        buf.put_u16(self.frame_type as u16);
        buf.put_u32(self.payload.len() as u32);
        buf.put_slice(&self.payload);
        buf.to_vec()
    }

    pub fn decode(buf: &[u8]) -> error::Result<Option<Self>> {
        if buf.len() < Self::HEADER_SIZE {
            return Ok(None);
        }

        let mut cursor = Cursor::new(buf);
        let frame_type_val = cursor.get_u16();
        let payload_len = cursor.get_u32();

        let frame_type =
            FrameType::from_u16(frame_type_val).ok_or_else(|| {
                error::ProtocolError::UnknownFrameType(frame_type_val)
            })?;

        let remaining = buf.len() - Self::HEADER_SIZE;
        if remaining < payload_len as usize {
            return Ok(None);
        }

        let payload = buf[Self::HEADER_SIZE..Self::HEADER_SIZE + payload_len as usize].to_vec();
        Ok(Some(Self { frame_type, payload }))
    }

    pub fn frame_type(&self) -> FrameType {
        self.frame_type
    }

    pub fn into_payload(self) -> Vec<u8> {
        self.payload
    }
}

/// Read a frame from a stream
pub async fn read_frame<R>(reader: &mut R) -> error::Result<Option<Frame>>
where
    R: tokio::io::AsyncReadExt + Unpin,
{
    // Read header
    let mut header = [0u8; Frame::HEADER_SIZE];
    match tokio::io::AsyncReadExt::read_exact(reader, &mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == tokio::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }

    let mut cursor = Cursor::new(header);
    let frame_type_val = cursor.get_u16();
    let payload_len = cursor.get_u32();

    let frame_type = FrameType::from_u16(frame_type_val)
        .ok_or_else(|| error::ProtocolError::UnknownFrameType(frame_type_val))?;

    // Read payload
    let mut payload = vec![0u8; payload_len as usize];
    if payload_len > 0 {
        tokio::io::AsyncReadExt::read_exact(reader, &mut payload).await?;
    }

    Ok(Some(Frame::new(frame_type, payload)))
}

/// Write a frame to a stream
pub async fn write_frame<W>(writer: &mut W, frame: &Frame) -> error::Result<()>
where
    W: tokio::io::AsyncWriteExt + Unpin,
{
    let data = frame.encode();
    tokio::io::AsyncWriteExt::write_all(writer, &data).await?;
    tokio::io::AsyncWriteExt::flush(writer).await?;
    Ok(())
}

// ============================================================
// Protocol Messages (JSON serialized)
// ============================================================

/// Device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub device_type: String,
    pub os_version: String,
    pub app_version: String,
    pub ip: String,
    pub port: u16,
}

/// Pairing request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRequest {
    pub pairing_id: String,
    pub device_id: String,
    pub device_name: String,
    pub public_key: String,
    pub capabilities: Capabilities,
}

/// Capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Capabilities {
    pub file_transfer: bool,
    pub screen_mirror: bool,
    pub remote_control: bool,
    pub clipboard_sync: bool,
}

/// Pairing response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingResponse {
    pub pairing_id: String,
    pub accepted: bool,
    pub error_message: Option<String>,
}

/// Chunk information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub index: u32,
    pub hash: Vec<u8>,
    pub size: u32,
}

/// File manifest request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileManifestRequest {
    pub file_id: String,
    pub file_name: String,
    pub total_size: u64,
    pub mime_type: String,
    pub chunk_size: u32,
    pub total_chunks: u32,
    pub chunks: Vec<ChunkInfo>,
}

/// File manifest response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileManifestResponse {
    pub file_id: String,
    pub accepted: bool,
    pub already_have_chunks: Vec<u32>,
    pub error_reason: Option<String>,
}

/// Chunk acknowledgment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkAck {
    pub file_id: String,
    pub index: u32,
    pub hash_match: bool,
}

/// Transfer complete
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferComplete {
    pub file_id: String,
    pub total_hash: Vec<u8>,
}

/// Transfer error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferError {
    pub file_id: String,
    pub error_code: u32,
    pub message: String,
}

/// Touch event type
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TouchType {
    Unknown = 0,
    Down = 1,
    Move = 2,
    Up = 3,
    Cancel = 4,
}

/// Touch event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchEvent {
    pub sequence_id: u64,
    pub timestamp_ms: u64,
    #[serde(rename = "type")]
    pub touch_type: TouchType,
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
    pub touch_major: f32,
    pub pointer_id: i32,
}

/// Key event type
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum KeyEventType {
    Unknown = 0,
    Down = 1,
    Up = 2,
    Text = 3,
}

/// Key event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEvent {
    pub sequence_id: u64,
    pub timestamp_ms: u64,
    #[serde(rename = "type")]
    pub key_type: KeyEventType,
    pub key_code: i32,
    pub text: Option<String>,
}

/// Clipboard content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardContent {
    pub timestamp_ms: u64,
    pub content: ClipboardData,
}

/// Clipboard data variants
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClipboardData {
    Text { text: String },
    Image { image_png: Vec<u8> },
    Url { url: String },
}

// ============================================================
// JSON Serialization Helpers
// ============================================================

impl DeviceInfo {
    /// Serialize to JSON bytes
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    /// Deserialize from JSON bytes
    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

impl PairingRequest {
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

impl PairingResponse {
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

impl FileManifestRequest {
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

impl FileManifestResponse {
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

impl TouchEvent {
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_encoding() {
        let frame = Frame::new(FrameType::PairingRequest, b"hello".to_vec());
        let encoded = frame.encode();

        assert_eq!(encoded.len(), 6 + 5);
        assert_eq!(u16::from_be_bytes([0x01, 0x01]), u16::from_be_bytes([encoded[0], encoded[1]]));
        assert_eq!(5u32, u32::from_be_bytes([encoded[2], encoded[3], encoded[4], encoded[5]]));
    }

    #[test]
    fn test_frame_decode() {
        let data = [0x01, 0x01, 0x00, 0x00, 0x00, 0x05, b'h', b'e', b'l', b'l', b'o'];
        let frame = Frame::decode(&data).unwrap().unwrap();

        assert_eq!(frame.frame_type, FrameType::PairingRequest);
        assert_eq!(frame.payload, b"hello");
    }

    #[test]
    fn test_frame_type_from_u16() {
        assert_eq!(FrameType::from_u16(0x0101), Some(FrameType::PairingRequest));
        assert_eq!(FrameType::from_u16(0x0203), Some(FrameType::ChunkData));
        assert_eq!(FrameType::from_u16(0xFFFF), None);
    }

    #[tokio::test]
    async fn test_read_write_frame() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let (reader, mut writer) = tokio::io::duplex(1024);

        let frame = Frame::new(FrameType::Heartbeat, vec![]);
        write_frame(&mut writer, &frame).await.unwrap();

        let mut reader = reader;
        let read_frame = read_frame(&mut reader).await.unwrap().unwrap();

        assert_eq!(read_frame.frame_type, FrameType::Heartbeat);
        assert!(read_frame.payload.is_empty());
    }

    #[test]
    fn test_json_serialization() {
        let device_info = DeviceInfo {
            id: "device-123".to_string(),
            name: "Test Device".to_string(),
            device_type: "phone".to_string(),
            os_version: "Android 13".to_string(),
            app_version: "1.0.0".to_string(),
            ip: "192.168.1.100".to_string(),
            port: 8080,
        };

        let json = device_info.to_json().unwrap();
        let decoded = DeviceInfo::from_json(&json).unwrap();

        assert_eq!(decoded.id, device_info.id);
        assert_eq!(decoded.name, device_info.name);
    }
}
