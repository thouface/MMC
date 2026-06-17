//! Clipboard frame encoding/decoding with transport layer integration.
//!
//! This module provides frame-based clipboard content encoding/decoding for
//! cross-device clipboard synchronization over TCP frames.

use mmc_protocol::{ClipboardContent, ClipboardData, Frame, FrameType};
use crate::error::{ClipboardError, Result};

/// Magic number prefix for clipboard frame content transport
pub const CLIPBOARD_MAGIC: u32 = 0x4D4D4343; // "MMCC"

/// Maximum clipboard content as a Frame with proper encoding
#[derive(Debug, Clone)]
pub struct ClipboardFrame {
    pub content: ClipboardContent,
    pub source_device_id: String,
}

impl ClipboardFrame {
    pub fn new(content: ClipboardContent, source_device_id: impl Into<String>) -> Self {
        Self {
            content,
            source_device_id: source_device_id.into(),
        }
    }

    /// Encode clipboard content + metadata into a Frame
    pub fn to_frame(&self) -> Result<Frame> {
        let payload = self.content.to_json().unwrap_or_default();
        Ok(Frame::new(FrameType::ClipboardContent, payload))
    }

    /// Decode a Frame into clipboard content
    pub fn from_frame(frame: &Frame) -> Result<Self> {
        if frame.frame_type != FrameType::ClipboardContent {
            return Err(ClipboardError::InvalidContent(format!(
                "Expected ClipboardContent frame, got {:?}",
                frame.frame_type
            )));
        }

        let content = ClipboardContent::from_json(&frame.payload)
            .map_err(|e| ClipboardError::Serialization(e.to_string()))?;

        Ok(Self {
            content,
            source_device_id: String::new(),
        })
    }

    /// Create a text clipboard frame with text content
    pub fn text(text: impl Into<String>, source_device_id: impl Into<String>) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self::new(
            ClipboardContent {
                timestamp_ms,
                content: ClipboardData::Text { text: text.into() },
            },
            source_device_id,
        )
    }

    /// Create a URL clipboard content
    pub fn url(url: impl Into<String>, source_device_id: impl Into<String>) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self::new(
            ClipboardContent {
                timestamp_ms,
                content: ClipboardData::Url { url: url.into() },
            },
            source_device_id,
        )
    }

    /// Create an image clipboard content
    pub fn image(image_png: Vec<u8>, source_device_id: impl Into<String>) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self::new(
            ClipboardContent {
                timestamp_ms,
                content: ClipboardData::Image { image_png },
            },
            source_device_id,
        )
    }
}

/// Clipboard content size helper
pub fn clipboard_data_size(data: &ClipboardData) -> usize {
    match data {
        ClipboardData::Text { text } => text.len(),
        ClipboardData::Image { image_png } => image_png.len(),
        ClipboardData::Url { url } => url.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_frame_new() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let content = ClipboardContent {
            timestamp_ms: ts,
            content: ClipboardData::Text { text: "Hello".to_string() },
        };
        let frame = ClipboardFrame::new(content, "device-1".to_string());
        assert_eq!(frame.source_device_id, "device-1");
        assert!(frame.content.timestamp_ms > 0);
    }

    #[test]
    fn test_clipboard_frame_text() {
        let frame = ClipboardFrame::text("Hello World", "device-1");
        assert!(frame.content.timestamp_ms > 0);
        assert!(matches!(
            frame.content.content, ClipboardData::Text { text: ref t } if t == "Hello World"));
        assert_eq!(frame.source_device_id, "device-1");
    }

    #[test]
    fn test_clipboard_frame_url() {
        let frame = ClipboardFrame::url("https://example.com", "device-2");
        assert!(matches!(
            frame.content.content, ClipboardData::Url { url: ref u } if u == "https://example.com"));
    }

    #[test]
    fn test_clipboard_frame_image() {
        let image_data = vec![0x89, 0x50, 0x4E, 0x47]; // PNG header
        let frame = ClipboardFrame::image(image_data.clone(), "device-3");
        assert!(matches!(
            frame.content.content, ClipboardData::Image { image_png: ref img } if img == &image_data));
    }

    #[test]
    fn test_clipboard_frame_roundtrip_text() {
        let frame = ClipboardFrame::text("Test clipboard", "source-1");
        assert_eq!(clipboard_data_size(&frame.content.content), 14);
    }

    #[test]
    fn test_clipboard_frame_roundtrip_url() {
        let frame = ClipboardFrame::url("https://test.org", "source-2");
        assert!(matches!(
            frame.content.content,
            ClipboardData::Url { url: ref u } if u == "https://test.org"));
    }

    #[test]
    fn test_clipboard_frame_image_data() {
        let image = vec![1, 2, 3, 4, 5];
        let frame = ClipboardFrame::image(image.clone(), "src-3");
        assert!(matches!(
            frame.content.content,
            ClipboardData::Image { image_png: ref img } if img == &image));
    }

    #[test]
    fn test_clipboard_data_size() {
        let text_data = ClipboardData::Text { text: "Hello".to_string() };
        assert_eq!(clipboard_data_size(&text_data), 5);

        let url_data = ClipboardData::Url { url: "https://a.com".to_string() };
        assert_eq!(clipboard_data_size(&url_data), 13);

        let image_data = ClipboardData::Image { image_png: vec![0u8; 100] };
        assert_eq!(clipboard_data_size(&image_data), 100);
    }
}
