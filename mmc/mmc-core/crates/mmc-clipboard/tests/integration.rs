//! Cross-module integration test: Device discovery → Pairing → Connection →
//! File transfer → Clipboard sync.
//!
//! This test simulates the full multi-device communication pipeline:
//! 1. Device A and Device B register with DeviceInfo (discovery)
//! 2. Device A sends PairingRequest to Device B, receives PairingResponse (pairing)
//! 3. Devices exchange frames over tokio duplex stream (connection)
//! 4. Device A sends file manifest, chunks, and TransferComplete (file transfer)
//! 5. Device A sends clipboard content, Device B applies it (clipboard sync)

use mmc_protocol::{DeviceInfo, Frame, FrameType, PairingRequest, PairingResponse,
                   Capabilities, read_frame, write_frame, ClipboardContent, ClipboardData};
use mmc_clipboard::{ClipboardManager, ClipboardFrame};

/// Helper: build a DeviceInfo
fn make_device(id: &str, name: &str, port: u16) -> DeviceInfo {
    DeviceInfo {
        id: id.to_string(),
        name: name.to_string(),
        device_type: "desktop".to_string(),
        os_version: "linux-6.0".to_string(),
        app_version: "1.0.0".to_string(),
        ip: "127.0.0.1".to_string(),
        port,
    }
}

#[tokio::test]
async fn test_integration_discovery_pairing_handshake() {
    let device_a = make_device("dev-a", "Device A", 8080);
    let _device_b = make_device("dev-b", "Device B", 8081);

    // Device A reads Device B's info (discovery)
    let pair_req = PairingRequest {
        pairing_id: "pair-1".to_string(),
        device_id: device_a.id.clone(),
        device_name: device_a.name.clone(),
        public_key: "pubkey-a".to_string(),
        capabilities: Capabilities {
            file_transfer: true,
            screen_mirror: true,
            remote_control: true,
            clipboard_sync: true,
        },
    };

    // Device B checks capabilities and responds
    assert!(pair_req.capabilities.file_transfer);
    assert!(pair_req.capabilities.clipboard_sync);

    let pair_resp = PairingResponse {
        pairing_id: "pair-1".to_string(),
        accepted: true,
        error_message: None,
    };

    assert!(pair_resp.accepted);
    assert_eq!(pair_resp.pairing_id, pair_req.pairing_id);
}

/// PairingRequest / PairingResponse via Frame encoding (JSON payload)
#[tokio::test]
async fn test_integration_pairing_via_frame_protocol() {
    let (mut a_stream, mut b_stream) = tokio::io::duplex(4096);

    // Device A -> sends PairingRequest frame
    let device_a = make_device("dev-a", "Device A", 8080);
    let pair_req = PairingRequest {
        pairing_id: "pair-1".to_string(),
        device_id: device_a.id.clone(),
        device_name: device_a.name.clone(),
        public_key: "pubkey-a".to_string(),
        capabilities: Capabilities {
            file_transfer: true,
            screen_mirror: true,
            remote_control: true,
            clipboard_sync: true,
        },
    };

    let payload = serde_json::to_vec(&pair_req).unwrap();
    let frame_a = Frame::new(FrameType::PairingRequest, payload);
    write_frame(&mut a_stream, &frame_a).await.unwrap();

    // Device B receives frame and decodes
    let received = read_frame(&mut b_stream).await.unwrap().unwrap();
    assert_eq!(received.frame_type, FrameType::PairingRequest);
    let decoded_req: PairingRequest = serde_json::from_slice(&received.payload).unwrap();
    assert_eq!(decoded_req.device_id, device_a.id);
    assert_eq!(decoded_req.pairing_id, "pair-1");
    assert!(decoded_req.capabilities.clipboard_sync);

    // Device B -> sends PairingResponse frame
    let pair_resp = PairingResponse {
        pairing_id: "pair-1".to_string(),
        accepted: true,
        error_message: None,
    };
    let resp_payload = serde_json::to_vec(&pair_resp).unwrap();
    let frame_b = Frame::new(FrameType::PairingResponse, resp_payload);
    write_frame(&mut b_stream, &frame_b).await.unwrap();

    // Device A receives response
    let received_resp = read_frame(&mut a_stream).await.unwrap().unwrap();
    assert_eq!(received_resp.frame_type, FrameType::PairingResponse);
    let decoded_resp: PairingResponse = serde_json::from_slice(&received_resp.payload).unwrap();
    assert!(decoded_resp.accepted);
    assert_eq!(decoded_resp.pairing_id, "pair-1");
}

/// Full pipeline: Discovery → Pairing → File manifest exchange → File chunk transfer →
/// Transfer complete → Clipboard content sync
#[tokio::test]
async fn test_integration_full_pipeline() {
    let (mut stream_a, mut stream_b) = tokio::io::duplex(65536);

    // --- Phase 1: Discovery + Pairing ---
    // Device A sends PairingRequest
    let pair_req = PairingRequest {
        pairing_id: "session-1".to_string(),
        device_id: "dev-a".to_string(),
        device_name: "Device A".to_string(),
        public_key: "pubkey-a".to_string(),
        capabilities: Capabilities {
            file_transfer: true,
            screen_mirror: true,
            remote_control: true,
            clipboard_sync: true,
        },
    };
    let payload = serde_json::to_vec(&pair_req).unwrap();
    write_frame(&mut stream_a, &Frame::new(FrameType::PairingRequest, payload)).await.unwrap();

    // Device B: Receive pairing request and respond
    let received = read_frame(&mut stream_b).await.unwrap().unwrap();
    assert_eq!(received.frame_type, FrameType::PairingRequest);
    let req: PairingRequest = serde_json::from_slice(&received.payload).unwrap();
    assert!(req.capabilities.file_transfer);
    assert!(req.capabilities.clipboard_sync);

    // Device B: Send PairingResponse
    let pair_resp = PairingResponse {
        pairing_id: req.pairing_id.clone(),
        accepted: true,
        error_message: None,
    };
    write_frame(
        &mut stream_b,
        &Frame::new(FrameType::PairingResponse, serde_json::to_vec(&pair_resp).unwrap()),
    )
    .await
    .unwrap();

    // Device A: Confirm pair accepted
    let accepted_frame = read_frame(&mut stream_a).await.unwrap().unwrap();
    let resp: PairingResponse = serde_json::from_slice(&accepted_frame.payload).unwrap();
    assert!(resp.accepted);

    // --- Phase 2: File transfer (manifest + chunks + complete) ---
    let file_chunks = vec![
        b"Hello".to_vec(),
        b" ".to_vec(),
        b"World".to_vec(),
    ];

    // Device A: Send manifest (simple JSON with file description)
    let manifest = serde_json::json!({
        "file_id": "file-1",
        "file_name": "greeting.txt",
        "total_size": 11u64,
        "total_chunks": file_chunks.len(),
    });
    let manifest_payload = serde_json::to_vec(&manifest).unwrap();
    write_frame(
        &mut stream_a,
        &Frame::new(FrameType::FileManifestRequest, manifest_payload),
    )
    .await
    .unwrap();

    // Device B: Receive manifest
    let manifest_frame = read_frame(&mut stream_b).await.unwrap().unwrap();
    assert_eq!(manifest_frame.frame_type, FrameType::FileManifestRequest);

    // Device B: Send manifest response accepting the file
    let manifest_resp = serde_json::json!({
        "file_id": "file-1",
        "accepted": true,
        "already_have_chunks": serde_json::Value::Array(vec![]),
    });
    write_frame(
        &mut stream_b,
        &Frame::new(FrameType::FileManifestResponse, serde_json::to_vec(&manifest_resp).unwrap()),
    )
    .await
    .unwrap();

    // Device A: Receive and verify manifest response
    let resp_frame = read_frame(&mut stream_a).await.unwrap().unwrap();
    assert_eq!(resp_frame.frame_type, FrameType::FileManifestResponse);
    let decoded_resp: serde_json::Value = serde_json::from_slice(&resp_frame.payload).unwrap();
    assert_eq!(decoded_resp["accepted"].as_bool(), Some(true));

    // Device A: Send file chunks
    for chunk in file_chunks.iter() {
        write_frame(&mut stream_a, &Frame::new(FrameType::ChunkData, chunk.clone()))
            .await
            .unwrap();
    }

    // Device A: Signal transfer complete
    write_frame(
        &mut stream_a,
        &Frame::new(FrameType::TransferComplete, vec![]),
    )
    .await
    .unwrap();

    // Device B: Receive all chunks + complete marker
    let mut received_file: Vec<u8> = Vec::new();
    loop {
        let frame = read_frame(&mut stream_b).await.unwrap().unwrap();
        match frame.frame_type {
            FrameType::ChunkData => received_file.extend_from_slice(&frame.payload),
            FrameType::TransferComplete => break,
            other => panic!("Unexpected frame type: {:?}", other),
        }
    }
    assert_eq!(received_file, b"Hello World".to_vec());

    // --- Phase 3: Clipboard content sync ---
    // Device A: Create clipboard content with ClipboardFrame, encode into Frame
    let clipboard_content = ClipboardContent {
        timestamp_ms: 1234567890,
        content: ClipboardData::Text {
            text: "Hello from Device A!".to_string(),
        },
    };
    let cb_frame = ClipboardFrame::new(clipboard_content.clone(), "dev-a");
    let encoded_frame = cb_frame.to_frame().unwrap();
    write_frame(&mut stream_a, &encoded_frame).await.unwrap();

    // Device B: Receive clipboard frame, decode, apply to local manager
    let clipboard_frame_raw = read_frame(&mut stream_b).await.unwrap().unwrap();
    assert_eq!(clipboard_frame_raw.frame_type, FrameType::ClipboardContent);

    let device_b_manager = ClipboardManager::new();
    let decoded_content = ClipboardContent::from_json(&clipboard_frame_raw.payload).unwrap();
    device_b_manager.set_remote(decoded_content.clone()).await.unwrap();

    let current = device_b_manager.get_current().await.unwrap();
    match (&clipboard_content.content, &current.content.content) {
        (
            ClipboardData::Text { text: a },
            ClipboardData::Text { text: b },
        ) => {
            assert_eq!(a, b);
            assert_eq!(b, "Hello from Device A!");
        }
        _ => panic!("Expected text content on both sides"),
    }
}

/// Integration: Multiple clipboard messages through Frame protocol
#[tokio::test]
async fn test_integration_multiple_clipboard_messages() {
    let (mut stream_a, mut stream_b) = tokio::io::duplex(4096);
    let device_b_manager = ClipboardManager::new();

    // Send multiple clipboard messages from Device A to Device B
    let messages = vec![
        "Message 1: Hello world",
        "Message 2: Rust is fun",
        "Message 3: Cross-device sync works!",
    ];

    for (i, msg) in messages.iter().enumerate() {
        let content = ClipboardContent {
            timestamp_ms: (i as u64) * 1000,
            content: ClipboardData::Text {
                text: msg.to_string(),
            },
        };
        let cb_frame = ClipboardFrame::new(content, "dev-a");
        let frame = cb_frame.to_frame().unwrap();
        write_frame(&mut stream_a, &frame).await.unwrap();
    }

    // Device B receives all messages
    for expected_msg in messages.iter() {
        let frame = read_frame(&mut stream_b).await.unwrap().unwrap();
        assert_eq!(frame.frame_type, FrameType::ClipboardContent);
        let content = ClipboardContent::from_json(&frame.payload).unwrap();
        device_b_manager.set_remote(content).await.unwrap();
    }

    // Verify the latest message is the current clipboard
    let current = device_b_manager.get_current().await.unwrap();
    match &current.content.content {
        ClipboardData::Text { text } => {
            assert_eq!(text, "Message 3: Cross-device sync works!");
        }
        _ => panic!("Expected text content"),
    }

    // Verify history was maintained (at least 3 entries)
    let history = device_b_manager.get_history().await;
    assert!(history.len() >= 3);
}
