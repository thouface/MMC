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
    // Screen Mirroring
    VideoFrame = 0x0401,
    AudioFrame = 0x0402,
    VideoConfig = 0x0403,
    AudioConfig = 0x0404,
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
            0x0401 => Some(Self::VideoFrame),
            0x0402 => Some(Self::AudioFrame),
            0x0403 => Some(Self::VideoConfig),
            0x0404 => Some(Self::AudioConfig),
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

/// Heartbeat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub timestamp_ms: u64,
    pub device_id: String,
}

/// Ping message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ping {
    pub timestamp_ms: u64,
}

/// Pong message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pong {
    pub timestamp_ms: u64,
    pub original_timestamp_ms: u64,
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

/// Video pixel format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PixelFormat {
    Unknown = 0,
    Rgba8888 = 1,
    Bgra8888 = 2,
    Rgb565 = 3,
    Yuv420p = 4,
    Nv12 = 5,
}

/// Video configuration message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoConfig {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub frame_rate: u32,
    pub codec: String,
    pub bitrate: u32,
}

/// Video frame message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoFrame {
    pub sequence_id: u64,
    pub timestamp_ms: u64,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub is_keyframe: bool,
    pub data: Vec<u8>,
}

/// Audio sample format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SampleFormat {
    Unknown = 0,
    U8 = 1,
    S16 = 2,
    S32 = 3,
    F32 = 4,
}

/// Audio configuration message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u32,
    pub sample_format: SampleFormat,
    pub codec: String,
    pub bitrate: u32,
}

/// Audio frame message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFrame {
    pub sequence_id: u64,
    pub timestamp_ms: u64,
    pub sample_rate: u32,
    pub channels: u32,
    pub sample_format: SampleFormat,
    pub data: Vec<u8>,
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

impl Heartbeat {
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

impl Ping {
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

impl Pong {
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

impl ClipboardContent {
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

impl KeyEvent {
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

impl VideoConfig {
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

impl VideoFrame {
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

impl AudioConfig {
    pub fn to_json(&self) -> error::Result<Vec<u8>> {
        serde_json::to_vec(self).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }

    pub fn from_json(data: &[u8]) -> error::Result<Self> {
        serde_json::from_slice(data).map_err(|e| error::ProtocolError::Serialization(e.to_string()))
    }
}

impl AudioFrame {
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

    #[test]
    fn test_heartbeat_json_roundtrip() {
        let heartbeat = Heartbeat {
            timestamp_ms: 1234567890,
            device_id: "device-abc".to_string(),
        };

        let json = heartbeat.to_json().unwrap();
        let decoded = Heartbeat::from_json(&json).unwrap();

        assert_eq!(decoded.timestamp_ms, heartbeat.timestamp_ms);
        assert_eq!(decoded.device_id, heartbeat.device_id);
    }

    #[test]
    fn test_ping_pong_json_roundtrip() {
        let ping = Ping { timestamp_ms: 1234567890 };
        let json = ping.to_json().unwrap();
        let decoded_ping = Ping::from_json(&json).unwrap();
        assert_eq!(decoded_ping.timestamp_ms, ping.timestamp_ms);

        let pong = Pong {
            timestamp_ms: 1234567895,
            original_timestamp_ms: 1234567890,
        };
        let json = pong.to_json().unwrap();
        let decoded_pong = Pong::from_json(&json).unwrap();
        assert_eq!(decoded_pong.timestamp_ms, pong.timestamp_ms);
        assert_eq!(decoded_pong.original_timestamp_ms, pong.original_timestamp_ms);
    }

    #[test]
    fn test_clipboard_content_json_roundtrip() {
        let clipboard_text = ClipboardContent {
            timestamp_ms: 1234567890,
            content: ClipboardData::Text { text: "Hello World".to_string() },
        };
        let json = clipboard_text.to_json().unwrap();
        let decoded = ClipboardContent::from_json(&json).unwrap();
        assert_eq!(decoded.timestamp_ms, clipboard_text.timestamp_ms);
        if let ClipboardData::Text { text } = decoded.content {
            assert_eq!(text, "Hello World");
        }

        let clipboard_url = ClipboardContent {
            timestamp_ms: 1234567891,
            content: ClipboardData::Url { url: "https://example.com".to_string() },
        };
        let json = clipboard_url.to_json().unwrap();
        let decoded = ClipboardContent::from_json(&json).unwrap();
        if let ClipboardData::Url { url } = decoded.content {
            assert_eq!(url, "https://example.com");
        }
    }

    #[test]
    fn test_video_frame_json_roundtrip() {
        let frame = VideoFrame {
            sequence_id: 42,
            timestamp_ms: 1234567890,
            width: 1920,
            height: 1080,
            pixel_format: PixelFormat::Rgba8888,
            is_keyframe: true,
            data: vec![0xAA, 0xBB, 0xCC, 0xDD],
        };
        let json = frame.to_json().unwrap();
        let decoded = VideoFrame::from_json(&json).unwrap();

        assert_eq!(decoded.sequence_id, frame.sequence_id);
        assert_eq!(decoded.width, frame.width);
        assert_eq!(decoded.height, frame.height);
        assert_eq!(decoded.pixel_format, frame.pixel_format);
        assert_eq!(decoded.is_keyframe, frame.is_keyframe);
        assert_eq!(decoded.data, frame.data);
    }

    #[test]
    fn test_video_config_json_roundtrip() {
        let config = VideoConfig {
            width: 1920,
            height: 1080,
            pixel_format: PixelFormat::Rgba8888,
            frame_rate: 30,
            codec: "raw".to_string(),
            bitrate: 5000000,
        };
        let json = config.to_json().unwrap();
        let decoded = VideoConfig::from_json(&json).unwrap();

        assert_eq!(decoded.width, config.width);
        assert_eq!(decoded.height, config.height);
        assert_eq!(decoded.pixel_format, config.pixel_format);
        assert_eq!(decoded.frame_rate, config.frame_rate);
        assert_eq!(decoded.codec, config.codec);
    }

    #[test]
    fn test_audio_frame_json_roundtrip() {
        let frame = AudioFrame {
            sequence_id: 7,
            timestamp_ms: 1234567890,
            sample_rate: 44100,
            channels: 2,
            sample_format: SampleFormat::S16,
            data: vec![0x01, 0x02, 0x03, 0x04],
        };
        let json = frame.to_json().unwrap();
        let decoded = AudioFrame::from_json(&json).unwrap();

        assert_eq!(decoded.sequence_id, frame.sequence_id);
        assert_eq!(decoded.sample_rate, frame.sample_rate);
        assert_eq!(decoded.channels, frame.channels);
        assert_eq!(decoded.sample_format, frame.sample_format);
        assert_eq!(decoded.data, frame.data);
    }

    #[test]
    fn test_key_event_json_roundtrip() {
        let event = KeyEvent {
            sequence_id: 100,
            timestamp_ms: 1234567890,
            key_type: KeyEventType::Down,
            key_code: 65,
            text: None,
        };
        let json = event.to_json().unwrap();
        let decoded = KeyEvent::from_json(&json).unwrap();

        assert_eq!(decoded.sequence_id, event.sequence_id);
        assert_eq!(decoded.key_type, event.key_type);
        assert_eq!(decoded.key_code, event.key_code);

        let event_text = KeyEvent {
            sequence_id: 101,
            timestamp_ms: 1234567891,
            key_type: KeyEventType::Text,
            key_code: 0,
            text: Some("Hello".to_string()),
        };
        let json = event_text.to_json().unwrap();
        let decoded = KeyEvent::from_json(&json).unwrap();
        assert_eq!(decoded.text, event_text.text);
    }

    #[test]
    fn test_touch_event_json_roundtrip() {
        let event = TouchEvent {
            sequence_id: 1,
            timestamp_ms: 1234567890,
            touch_type: TouchType::Down,
            x: 100.5,
            y: 200.3,
            pressure: 0.8,
            touch_major: 5.0,
            pointer_id: 0,
        };
        let json = event.to_json().unwrap();
        let decoded = TouchEvent::from_json(&json).unwrap();

        assert_eq!(decoded.sequence_id, event.sequence_id);
        assert_eq!(decoded.touch_type, event.touch_type);
        assert_eq!(decoded.x, event.x);
        assert_eq!(decoded.y, event.y);
        assert_eq!(decoded.pressure, event.pressure);
    }

    #[test]
    fn test_new_frame_types() {
        assert_eq!(FrameType::from_u16(0x0401), Some(FrameType::VideoFrame));
        assert_eq!(FrameType::from_u16(0x0402), Some(FrameType::AudioFrame));
        assert_eq!(FrameType::from_u16(0x0403), Some(FrameType::VideoConfig));
        assert_eq!(FrameType::from_u16(0x0404), Some(FrameType::AudioConfig));
    }
}
