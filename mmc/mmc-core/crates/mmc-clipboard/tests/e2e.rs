//! End-to-end clipboard synchronization integration test.
//!
//! This test simulates the full clipboard synchronization pipeline:
//! 1. Device A creates clipboard content (text/URL/image)
//! 2. Content is encoded into a Frame with proper encoding (JSON)
//! 3. Frame is sent through the transport mechanism
//! 4. Frame is received by Device B
//! 5. Content is decoded and applied to local clipboard on Device B
//! 6. Content hash/size is verified

use std::sync::Arc;
use mmc_protocol::{ClipboardContent, ClipboardData, Frame, FrameType};
use mmc_clipboard::{ClipboardManager, ClipboardFrame};

#[tokio::test]
async fn test_e2e_clipboard_text_sync() {
    // Setup: Two devices with clipboard managers
    let device_a = Arc::new(ClipboardManager::new());
    let device_b = Arc::new(ClipboardManager::new());

    // Device A: Set local clipboard content
    let content_a = ClipboardContent {
        timestamp_ms: 1234567890,
        content: ClipboardData::Text { text: "Hello from Device A".to_string() },
    };
    device_a.set_local(content_a.clone()).await.unwrap();

    // Verify Device A has content
    let a_current = device_a.get_current().await;
    assert!(a_current.is_some());
    let a_entry = a_current.unwrap();
    assert_eq!(a_entry.content_size, 19);

    // Step 1: Encode the clipboard content into a Frame
    let clipboard_frame = ClipboardFrame::new(content_a.clone(), "device-a");
    let frame = clipboard_frame.to_frame().unwrap();

    // Verify the frame has the correct type
    assert_eq!(frame.frame_type, FrameType::ClipboardContent);
    // Verify the JSON payload is non-empty
    assert!(!frame.payload.is_empty());

    // Step 2: Simulate sending through transport (encode/decode frame)
    let encoded_data = frame.encode();
    assert!(!encoded_data.is_empty());

    // Step 3: Decode the frame on Device B
    let decoded_frame = Frame::decode(&encoded_data).unwrap().unwrap();
    assert_eq!(decoded_frame.frame_type, FrameType::ClipboardContent);

    // Step 4: Decode clipboard content from the frame (simulating receive on Device B)
    let content_b = ClipboardContent::from_json(&decoded_frame.payload).unwrap();

    // Step 5: Apply to Device B's local clipboard as "from remote"
    device_b.set_remote(content_b.clone()).await.unwrap();

    // Verify Device B has received the content
    let b_current = device_b.get_current().await;
    assert!(b_current.is_some());
    let b_entry = b_current.unwrap();
    assert_eq!(b_entry.content_size, 19);

    // Verify content match
    match (&content_a.content, &b_entry.content.content) {
        (ClipboardData::Text { text: a }, ClipboardData::Text { text: b }) => {
            assert_eq!(a, b);
        }
        _ => panic!("Expected text content on both sides"),
    }
}

#[tokio::test]
async fn test_e2e_clipboard_url_sync() {
    let device_a = ClipboardManager::new();
    let device_b = ClipboardManager::new();

    let content_a = ClipboardContent {
        timestamp_ms: 9876543210,
        content: ClipboardData::Url { url: "https://rust-lang.org".to_string() },
    };
    device_a.set_local(content_a.clone()).await.unwrap();

    // Encode
    let clipboard_frame = ClipboardFrame::new(content_a.clone(), "device-a");
    let frame = clipboard_frame.to_frame().unwrap();
    let encoded = frame.encode();

    // Decode
    let decoded = Frame::decode(&encoded).unwrap().unwrap();
    assert_eq!(decoded.frame_type, FrameType::ClipboardContent);

    let content_b = ClipboardContent::from_json(&decoded.payload).unwrap();

    // Apply to device B
    device_b.set_remote(content_b).await.unwrap();

    // Verify
    let current_b = device_b.get_current().await.unwrap();
    assert_eq!(current_b.content_size, 21); // "https://rust-lang.org" is 21 chars

    match current_b.content.content {
        ClipboardData::Url { url } => assert_eq!(url, "https://rust-lang.org"),
        _ => panic!("Expected URL content"),
    }
}

#[tokio::test]
async fn test_e2e_clipboard_image_sync() {
    let device_a = ClipboardManager::new();
    let device_b = ClipboardManager::new();

    // Simulate small PNG data
    let png_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4, 5];
    let content_a = ClipboardContent {
        timestamp_ms: 1122334455,
        content: ClipboardData::Image { image_png: png_data.clone() },
    };

    device_a.set_local(content_a.clone()).await.unwrap();

    // Encode
    let clipboard_frame = ClipboardFrame::new(content_a.clone(), "device-a");
    let frame = clipboard_frame.to_frame().unwrap();
    let encoded = frame.encode();

    // Decode
    let decoded = Frame::decode(&encoded).unwrap().unwrap();
    let content_b = ClipboardContent::from_json(&decoded.payload).unwrap();

    device_b.set_remote(content_b).await.unwrap();

    // Verify: The image bytes are preserved
    let current_b = device_b.get_current().await.unwrap();
    match current_b.content.content {
        ClipboardData::Image { image_png } => assert_eq!(image_png, png_data),
        _ => panic!("Expected image content"),
    }
}

#[tokio::test]
async fn test_e2e_clipboard_manager_history() {
    let manager = ClipboardManager::new();

    // Simulate multiple clipboard changes
    let contents = vec![
        ClipboardData::Text { text: "First copy".to_string() },
        ClipboardData::Url { url: "https://first.com".to_string() },
        ClipboardData::Text { text: "Second copy".to_string() },
        ClipboardData::Text { text: "Third copy".to_string() },
        ClipboardData::Image { image_png: vec![1, 2, 3, 4] },
    ];

    for (i, c) in contents.iter().enumerate() {
        let content = ClipboardContent {
            timestamp_ms: (i as u64 + 1) * 1000,
            content: c.clone(),
        };
        manager.set_local(content).await.unwrap();
    }

    // History should have at least 5 entries
    let history = manager.get_history().await;
    assert_eq!(history.len(), 5);

    // Current should be the image
    let current = manager.get_current().await.unwrap();
    assert_eq!(current.content_size, 4);
    assert!(matches!(current.content.content, ClipboardData::Image { .. }));
}

#[tokio::test]
async fn test_e2e_clipboard_frame_encode_decode() {
    // Test the raw frame encoding / decoding
    let clipboard_frame = ClipboardFrame::text("Test content", "source-device");

    // Encode to a Frame
    let frame = clipboard_frame.to_frame().unwrap();
    assert_eq!(frame.frame_type, FrameType::ClipboardContent);

    // Frame to bytes
    let encoded = frame.encode();
    assert!(encoded.len() > Frame::HEADER_SIZE);

    // Decode bytes back to frame
    let decoded = Frame::decode(&encoded).unwrap().unwrap();
    assert_eq!(decoded.frame_type, FrameType::ClipboardContent);

    // Decode clipboard from frame
    let content = ClipboardContent::from_json(&decoded.payload).unwrap();

    // Verify content is preserved
    match content.content {
        ClipboardData::Text { text } => assert_eq!(text, "Test content"),
        _ => panic!("Expected text"),
    }
}

#[tokio::test]
async fn test_e2e_clipboard_invalid_frame_type() {
    // Create a non-clipboard frame
    let frame = Frame::new(FrameType::TouchEvent, b"not clipboard".to_vec());
    let encoded = frame.encode();
    let decoded = Frame::decode(&encoded).unwrap().unwrap();
    assert_eq!(decoded.frame_type, FrameType::TouchEvent);

    // Try to parse as clipboard - should fail
    let result = ClipboardContent::from_json(&decoded.payload);
    assert!(result.is_err()); // TouchEvent payload is not valid clipboard JSON
}

#[tokio::test]
async fn test_e2e_clipboard_clear() {
    let manager = ClipboardManager::new();

    // Set content
    let content = ClipboardContent {
        timestamp_ms: 1000,
        content: ClipboardData::Text { text: "Temp".to_string() },
    };
    manager.set_local(content).await.unwrap();
    assert!(!manager.is_empty().await);

    // Clear
    manager.clear().await.unwrap();
    assert!(manager.is_empty().await);

    // History should still contain entries (clear only clears current)
    // Actually looking at manager impl, it has no explicit history clearing
    // history remains after clear as we don't clear it
}

#[tokio::test]
async fn test_e2e_clipboard_content_sizes() {
    let manager = ClipboardManager::new();

    // Test various sizes
    let test_cases = vec![
        (ClipboardData::Text { text: String::new() }, 0),
        (ClipboardData::Text { text: "A".to_string() }, 1),
        (ClipboardData::Text { text: "Hello".to_string() }, 5),
        (ClipboardData::Text { text: "R".repeat(1000) }, 1000),
        (ClipboardData::Url { url: String::new() }, 0),
        (ClipboardData::Url { url: "https://a.com".to_string() }, 13),
        (ClipboardData::Image { image_png: Vec::new() }, 0),
        (ClipboardData::Image { image_png: vec![0u8; 256] }, 256),
    ];

    for (data, expected_size) in test_cases {
        let content = ClipboardContent {
            timestamp_ms: 1000,
            content: data,
        };
        manager.set_local(content).await.unwrap();

        let entry = manager.get_current().await.unwrap();
        assert_eq!(entry.content_size, expected_size, "Content size mismatch");
    }
}

#[tokio::test]
async fn test_e2e_clipboard_content_size_boundary() {
    let manager = ClipboardManager::with_limits(10, 1000); // 1000 bytes limit

    // Valid small content
    let small = ClipboardContent {
        timestamp_ms: 1000,
        content: ClipboardData::Text { text: "small".to_string() },
    };
    assert!(manager.set_local(small).await.is_ok());

    // Valid exactly at limit
    let at_limit = ClipboardContent {
        timestamp_ms: 2000,
        content: ClipboardData::Text { text: "X".repeat(1000) },
    };
    assert!(manager.set_local(at_limit).await.is_ok());

    // Invalid over limit
    let over_limit = ClipboardContent {
        timestamp_ms: 3000,
        content: ClipboardData::Text { text: "Y".repeat(1001) },
    };
    assert!(manager.set_local(over_limit).await.is_err());
}

#[test]
fn test_clipboard_content_json_roundtrip() {
    // Test JSON encoding directly on content
    let content = ClipboardContent {
        timestamp_ms: 12345,
        content: ClipboardData::Text { text: "Roundtrip test".to_string() },
    };

    let json = content.to_json().unwrap();
    let decoded = ClipboardContent::from_json(&json).unwrap();

    assert_eq!(decoded.timestamp_ms, 12345);
    match decoded.content {
        ClipboardData::Text { text } => assert_eq!(text, "Roundtrip test"),
        _ => panic!("Expected text"),
    }
}

#[test]
fn test_clipboard_content_json_url_roundtrip() {
    let content = ClipboardContent {
        timestamp_ms: 98765,
        content: ClipboardData::Url { url: "https://test.com/path".to_string() },
    };

    let json = content.to_json().unwrap();
    let decoded = ClipboardContent::from_json(&json).unwrap();

    match decoded.content {
        ClipboardData::Url { url } => assert_eq!(url, "https://test.com/path"),
        _ => panic!("Expected URL"),
    }
}

#[test]
fn test_clipboard_frame_to_frame_roundtrip() {
    let frame = ClipboardFrame::text("Hello World", "device-1");
    let protocol_frame = frame.to_frame().unwrap();
    assert_eq!(protocol_frame.frame_type, FrameType::ClipboardContent);

    let decoded_clipboard = ClipboardFrame::from_frame(&protocol_frame).unwrap();
    match decoded_clipboard.content.content {
        ClipboardData::Text { text } => assert_eq!(text, "Hello World"),
        _ => panic!("Expected text content"),
    }
}

#[test]
fn test_clipboard_frame_from_wrong_frame_type() {
    // Create a non-clipboard frame
    let bad_frame = Frame::new(FrameType::PairingRequest, b"some bytes".to_vec());

    // Try to parse it as clipboard - should fail due to frame type check
    let parsed = ClipboardFrame::from_frame(&bad_frame);
    assert!(parsed.is_err());
}
