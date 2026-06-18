//! Frame-based data transmission.
//!
//! This module provides frame encoding/decoding for reliable data transmission
//! over TCP connections. Each frame has a fixed header containing:
//! - Magic number (4 bytes): 0x4D4D4330 ("MMC0")
//! - Frame type (1 byte): Data, Heartbeat, Ack, etc.
//! - Payload length (4 bytes): Size of the payload
//! - Sequence ID (4 bytes): For ordering and acknowledgment
//! - Total header size: 13 bytes

use bytes::{Bytes, BytesMut, Buf, BufMut};
use std::io::{Read, Write};
use crate::error::{Result, TransportError};

/// Magic number for MMC frames: "MMC0" in ASCII.
const MAGIC: u32 = 0x4D4D4330;

/// Maximum frame payload size (10 MB).
const MAX_PAYLOAD_SIZE: usize = 10 * 1024 * 1024;

/// Frame header size (13 bytes).
const HEADER_SIZE: usize = 13;

/// Frame types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    /// Data frame containing video/audio/input data.
    Data = 0x01,
    /// Heartbeat frame for connection keepalive.
    Heartbeat = 0x02,
    /// Acknowledgment frame.
    Ack = 0x03,
    /// Error frame indicating transmission failure.
    Error = 0x04,
    /// Control frame for connection management.
    Control = 0x05,
    /// File transfer chunk frame.
    FileChunk = 0x06,
}

impl TryFrom<u8> for FrameType {
    type Error = TransportError;
    
    fn try_from(value: u8) -> Result<Self> {
        match value {
            0x01 => Ok(FrameType::Data),
            0x02 => Ok(FrameType::Heartbeat),
            0x03 => Ok(FrameType::Ack),
            0x04 => Ok(FrameType::Error),
            0x05 => Ok(FrameType::Control),
            0x06 => Ok(FrameType::FileChunk),
            _ => Err(TransportError::InvalidFrameHeader),
        }
    }
}

/// Frame header structure.
#[derive(Debug, Clone)]
pub struct FrameHeader {
    pub frame_type: FrameType,
    pub payload_len: u32,
    pub sequence_id: u32,
}

impl FrameHeader {
    /// Create a new frame header.
    pub fn new(frame_type: FrameType, payload_len: u32, sequence_id: u32) -> Self {
        Self {
            frame_type,
            payload_len,
            sequence_id,
        }
    }
    
    /// Encode header to bytes.
    pub fn encode(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(HEADER_SIZE);
        buf.put_u32(MAGIC);
        buf.put_u8(self.frame_type as u8);
        buf.put_u32(self.payload_len);
        buf.put_u32(self.sequence_id);
        buf.freeze()
    }
    
    /// Decode header from bytes.
    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_SIZE {
            return Err(TransportError::IncompleteFrame(HEADER_SIZE, data.len()));
        }
        
        let mut buf = &data[..HEADER_SIZE];
        let magic = buf.get_u32();
        if magic != MAGIC {
            return Err(TransportError::InvalidFrameHeader);
        }
        
        let frame_type = buf.get_u8().try_into()?;
        let payload_len = buf.get_u32();
        let sequence_id = buf.get_u32();
        
        if payload_len as usize > MAX_PAYLOAD_SIZE {
            return Err(TransportError::FrameTooLarge(payload_len as usize, MAX_PAYLOAD_SIZE));
        }
        
        Ok(Self {
            frame_type,
            payload_len,
            sequence_id,
        })
    }
}

/// Complete frame with header and payload.
#[derive(Debug, Clone)]
pub struct Frame {
    pub header: FrameHeader,
    pub payload: Bytes,
}

impl Frame {
    /// Create a new data frame.
    pub fn data(sequence_id: u32, payload: Bytes) -> Result<Self> {
        let len = payload.len();
        if len > MAX_PAYLOAD_SIZE {
            return Err(TransportError::FrameTooLarge(len, MAX_PAYLOAD_SIZE));
        }
        Ok(Self {
            header: FrameHeader::new(FrameType::Data, len as u32, sequence_id),
            payload,
        })
    }
    
    /// Create a heartbeat frame.
    pub fn heartbeat(sequence_id: u32) -> Self {
        Self {
            header: FrameHeader::new(FrameType::Heartbeat, 0, sequence_id),
            payload: Bytes::new(),
        }
    }
    
    /// Create an ack frame.
    pub fn ack(sequence_id: u32, acked_id: u32) -> Self {
        let payload = Bytes::copy_from_slice(&acked_id.to_le_bytes());
        Self {
            header: FrameHeader::new(FrameType::Ack, 4, sequence_id),
            payload,
        }
    }
    
    /// Create a file chunk frame.
    pub fn file_chunk(sequence_id: u32, payload: Bytes) -> Result<Self> {
        let len = payload.len();
        if len > MAX_PAYLOAD_SIZE {
            return Err(TransportError::FrameTooLarge(len, MAX_PAYLOAD_SIZE));
        }
        Ok(Self {
            header: FrameHeader::new(FrameType::FileChunk, len as u32, sequence_id),
            payload,
        })
    }
    
    /// Encode the complete frame to bytes.
    pub fn encode(&self) -> Bytes {
        let header_bytes = self.header.encode();
        let mut buf = BytesMut::with_capacity(HEADER_SIZE + self.payload.len());
        buf.put_slice(&header_bytes);
        buf.put_slice(&self.payload);
        buf.freeze()
    }
    
    /// Get total frame size (header + payload).
    pub fn total_size(&self) -> usize {
        HEADER_SIZE + self.payload.len()
    }
    
    /// Check if this is a heartbeat frame.
    pub fn is_heartbeat(&self) -> bool {
        self.header.frame_type == FrameType::Heartbeat
    }
    
    /// Check if this is a data frame.
    pub fn is_data(&self) -> bool {
        self.header.frame_type == FrameType::Data
    }
}

/// Frame codec for encoding/decoding frames from streams.
pub struct FrameCodec {
    buffer: BytesMut,
    max_frame_size: usize,
}

impl FrameCodec {
    /// Create a new frame codec.
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::with_capacity(64 * 1024),
            max_frame_size: MAX_PAYLOAD_SIZE + HEADER_SIZE,
        }
    }
    
    /// Create a codec with custom max frame size.
    pub fn with_max_size(max_frame_size: usize) -> Self {
        Self {
            buffer: BytesMut::with_capacity(max_frame_size),
            max_frame_size,
        }
    }
    
    /// Read a frame from a stream.
    pub fn read_frame<R: Read>(&mut self, reader: &mut R) -> Result<Option<Frame>> {
        // Try to read header first
        if self.buffer.len() < HEADER_SIZE {
            let to_read = HEADER_SIZE - self.buffer.len();
            let mut temp_buf = vec![0u8; to_read];
            
            let n = reader.read(&mut temp_buf)?;
            if n == 0 {
                return Ok(None); // No data available
            }
            self.buffer.extend_from_slice(&temp_buf[..n]);
            
            if self.buffer.len() < HEADER_SIZE {
                return Ok(None); // Incomplete header
            }
        }
        
        // Decode header
        let header = FrameHeader::decode(&self.buffer)?;
        let total_len = HEADER_SIZE + header.payload_len as usize;
        
        // Check max size
        if total_len > self.max_frame_size {
            return Err(TransportError::FrameTooLarge(total_len, self.max_frame_size));
        }
        
        // Read remaining payload if needed
        if self.buffer.len() < total_len {
            let remaining = total_len - self.buffer.len();
            let mut temp_buf = vec![0u8; remaining];
            let n = reader.read(&mut temp_buf)?;
            if n == 0 {
                return Ok(None);
            }
            self.buffer.extend_from_slice(&temp_buf[..n]);
            
            if self.buffer.len() < total_len {
                return Ok(None); // Incomplete payload
            }
        }
        
        // Extract frame
        let header = FrameHeader::decode(&self.buffer)?;
        let payload = self.buffer.split_to(HEADER_SIZE + header.payload_len as usize);
        let payload_bytes = payload.freeze();
        let payload_data = payload_bytes.slice(HEADER_SIZE..);
        
        // Clear consumed bytes
        self.buffer.clear();
        
        Ok(Some(Frame {
            header,
            payload: payload_data,
        }))
    }
    
    /// Write a frame to a stream.
    pub fn write_frame<W: Write>(&mut self, writer: &mut W, frame: &Frame) -> Result<()> {
        let encoded = frame.encode();
        writer.write_all(&encoded)?;
        writer.flush()?;
        Ok(())
    }
    
    /// Get buffer size.
    pub fn buffer_size(&self) -> usize {
        self.buffer.len()
    }
    
    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for FrameCodec {
    fn default() -> Self {
        Self::new()
    }
}

/// Frame sender for sending frames over a connection.
pub struct FrameSender<W: Write> {
    writer: W,
    codec: FrameCodec,
    sequence_id: u32,
}

impl<W: Write> FrameSender<W> {
    /// Create a new frame sender.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            codec: FrameCodec::new(),
            sequence_id: 0,
        }
    }
    
    /// Send a data frame.
    pub fn send_data(&mut self, payload: Bytes) -> Result<u32> {
        self.sequence_id += 1;
        let frame = Frame::data(self.sequence_id, payload)?;
        self.codec.write_frame(&mut self.writer, &frame)?;
        Ok(self.sequence_id)
    }
    
    /// Send a heartbeat frame.
    pub fn send_heartbeat(&mut self) -> Result<u32> {
        self.sequence_id += 1;
        let frame = Frame::heartbeat(self.sequence_id);
        self.codec.write_frame(&mut self.writer, &frame)?;
        Ok(self.sequence_id)
    }
    
    /// Send an ack frame.
    pub fn send_ack(&mut self, acked_id: u32) -> Result<u32> {
        self.sequence_id += 1;
        let frame = Frame::ack(self.sequence_id, acked_id);
        self.codec.write_frame(&mut self.writer, &frame)?;
        Ok(self.sequence_id)
    }
    
    /// Send a file chunk frame.
    pub fn send_file_chunk(&mut self, payload: Bytes) -> Result<u32> {
        self.sequence_id += 1;
        let frame = Frame::file_chunk(self.sequence_id, payload)?;
        self.codec.write_frame(&mut self.writer, &frame)?;
        Ok(self.sequence_id)
    }
    
    /// Send a raw frame.
    pub fn send_frame(&mut self, frame: &Frame) -> Result<()> {
        self.codec.write_frame(&mut self.writer, frame)?;
        Ok(())
    }
    
    /// Get current sequence ID.
    pub fn sequence_id(&self) -> u32 {
        self.sequence_id
    }
}

/// Frame receiver for receiving frames from a connection.
pub struct FrameReceiver<R: Read> {
    reader: R,
    codec: FrameCodec,
}

impl<R: Read> FrameReceiver<R> {
    /// Create a new frame receiver.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            codec: FrameCodec::new(),
        }
    }
    
    /// Receive a frame.
    pub fn receive_frame(&mut self) -> Result<Option<Frame>> {
        self.codec.read_frame(&mut self.reader)
    }
    
    /// Receive a frame, blocking until one is available.
    pub fn receive_frame_blocking(&mut self) -> Result<Frame> {
        loop {
            match self.receive_frame()? {
                Some(frame) => return Ok(frame),
                None => continue,
            }
        }
    }
    
    /// Get buffer size.
    pub fn buffer_size(&self) -> usize {
        self.codec.buffer_size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    
    #[test]
    fn test_frame_type_conversion() {
        assert_eq!(FrameType::try_from(0x01).unwrap(), FrameType::Data);
        assert_eq!(FrameType::try_from(0x02).unwrap(), FrameType::Heartbeat);
        assert_eq!(FrameType::try_from(0x03).unwrap(), FrameType::Ack);
        assert!(FrameType::try_from(0xFF).is_err());
    }
    
    #[test]
    fn test_frame_header_encode_decode() {
        let header = FrameHeader::new(FrameType::Data, 100, 42);
        let encoded = header.encode();
        assert_eq!(encoded.len(), HEADER_SIZE);
        
        let decoded = FrameHeader::decode(&encoded).unwrap();
        assert_eq!(decoded.frame_type, FrameType::Data);
        assert_eq!(decoded.payload_len, 100);
        assert_eq!(decoded.sequence_id, 42);
    }
    
    #[test]
    fn test_frame_header_invalid_magic() {
        let mut buf = BytesMut::with_capacity(HEADER_SIZE);
        buf.put_u32(0x12345678); // Invalid magic
        buf.put_u8(0x01);
        buf.put_u32(100);
        buf.put_u32(42);
        
        let result = FrameHeader::decode(&buf);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_frame_header_incomplete() {
        let data = [0u8; 5];
        let result = FrameHeader::decode(&data);
        assert!(matches!(result, Err(TransportError::IncompleteFrame(13, 5))));
    }
    
    #[test]
    fn test_data_frame() {
        let payload = Bytes::from("Hello, World!");
        let frame = Frame::data(1, payload.clone()).unwrap();
        
        assert_eq!(frame.header.frame_type, FrameType::Data);
        assert_eq!(frame.header.sequence_id, 1);
        assert_eq!(frame.payload.len(), payload.len());
        assert!(frame.is_data());
        assert!(!frame.is_heartbeat());
    }
    
    #[test]
    fn test_heartbeat_frame() {
        let frame = Frame::heartbeat(100);
        assert_eq!(frame.header.frame_type, FrameType::Heartbeat);
        assert_eq!(frame.header.payload_len, 0);
        assert!(frame.is_heartbeat());
        assert!(!frame.is_data());
    }
    
    #[test]
    fn test_ack_frame() {
        let frame = Frame::ack(50, 42);
        assert_eq!(frame.header.frame_type, FrameType::Ack);
        assert_eq!(frame.header.payload_len, 4);
        
        let acked_id = u32::from_le_bytes(frame.payload[..4].try_into().unwrap());
        assert_eq!(acked_id, 42);
    }
    
    #[test]
    fn test_frame_encode_decode_roundtrip() {
        let payload = Bytes::from("Test payload data");
        let frame = Frame::data(123, payload.clone()).unwrap();
        let encoded = frame.encode();
        
        // Decode header
        let header = FrameHeader::decode(&encoded).unwrap();
        assert_eq!(header.frame_type, FrameType::Data);
        assert_eq!(header.sequence_id, 123);
        assert_eq!(header.payload_len as usize, payload.len());
        
        // Payload should match
        let payload_data = &encoded[HEADER_SIZE..];
        assert_eq!(payload_data, &payload[..]);
    }
    
    #[test]
    fn test_frame_too_large() {
        let large_payload = Bytes::from(vec![0u8; MAX_PAYLOAD_SIZE + 1]);
        let result = Frame::data(1, large_payload);
        assert!(matches!(result, Err(TransportError::FrameTooLarge(_, MAX_PAYLOAD_SIZE))));
    }
    
    #[test]
    fn test_frame_codec_roundtrip() {
        let payload = Bytes::from("Hello, MMC!");
        let frame = Frame::data(1, payload.clone()).unwrap();
        let encoded = frame.encode();
        
        let mut cursor = Cursor::new(encoded.to_vec());
        let mut codec = FrameCodec::new();
        
        let decoded = codec.read_frame(&mut cursor).unwrap().unwrap();
        assert_eq!(decoded.header.frame_type, FrameType::Data);
        assert_eq!(decoded.header.sequence_id, 1);
        assert_eq!(decoded.payload, payload);
    }
    
    #[test]
    fn test_frame_sender_receiver() {
        // First, encode some frames into the cursor's underlying data
        let mut write_cursor = Cursor::new(Vec::new());
        let mut sender = FrameSender::new(&mut write_cursor);
        
        let id1 = sender.send_data(Bytes::from("First")).unwrap();
        let id2 = sender.send_data(Bytes::from("Second")).unwrap();
        let id3 = sender.send_heartbeat().unwrap();
        
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
        
        // Now read from the written data
        let written_data = write_cursor.into_inner();
        let mut read_cursor = Cursor::new(written_data);
        let mut receiver = FrameReceiver::new(&mut read_cursor);
        
        let frame1 = receiver.receive_frame().unwrap().unwrap();
        assert_eq!(frame1.header.sequence_id, 1);
        assert_eq!(frame1.payload, Bytes::from("First"));
        
        let frame2 = receiver.receive_frame().unwrap().unwrap();
        assert_eq!(frame2.header.sequence_id, 2);
        assert_eq!(frame2.payload, Bytes::from("Second"));
        
        let frame3 = receiver.receive_frame().unwrap().unwrap();
        assert!(frame3.is_heartbeat());
        assert_eq!(frame3.header.sequence_id, 3);
    }
    
    #[test]
    fn test_frame_total_size() {
        let payload = Bytes::from("Test");
        let frame = Frame::data(1, payload).unwrap();
        assert_eq!(frame.total_size(), HEADER_SIZE + 4);
    }
    
    #[test]
    fn test_frame_codec_default() {
        let codec = FrameCodec::default();
        assert_eq!(codec.buffer_size(), 0);
    }
    
    #[test]
    fn test_file_chunk_frame() {
        let payload = Bytes::from(vec![1, 2, 3, 4, 5]);
        let frame = Frame::file_chunk(10, payload.clone()).unwrap();
        assert_eq!(frame.header.frame_type, FrameType::FileChunk);
        assert_eq!(frame.header.sequence_id, 10);
        assert_eq!(frame.payload, payload);
    }
}