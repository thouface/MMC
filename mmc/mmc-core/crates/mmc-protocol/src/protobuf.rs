//! Protobuf message definitions for MMC protocol
//!
//! These types provide efficient binary serialization using prost.
//! Generated from proto/mmc/v1/mmc.proto

use serde::{Deserialize, Serialize};

// Re-export generated types
include!("generated/mmc.v1.rs");

/// Device type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum ProtoDeviceType {
    #[default]
    Unknown = 0,
    Phone = 1,
    Tablet = 2,
    Pc = 3,
    Tv = 4,
    Wearable = 5,
}

/// Transfer error reason
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum ProtoTransferErrorReason {
    #[default]
    None = 0,
    DiskFull = 1,
    PermissionDenied = 2,
    UnsupportedType = 3,
    SizeLimitExceeded = 4,
}

/// Touch event type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum ProtoTouchType {
    #[default]
    Unknown = 0,
    Down = 1,
    Move = 2,
    Up = 3,
    Cancel = 4,
}

/// Key event type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(i32)]
pub enum ProtoKeyEventType {
    #[default]
    Unknown = 0,
    Down = 1,
    Up = 2,
    Text = 3,
}
#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use prost::Message;

    #[test]
    fn test_device_info_roundtrip() {
        let device = DeviceInfo {
            device_id: "device-123".to_string(),
            device_name: "Test Device".to_string(),
            device_type: DeviceType::Phone as i32,
            os_version: "Android 13".to_string(),
            app_version: "1.0.0".to_string(),
            ip_address: "192.168.1.100".to_string(),
            port: 8080,
            public_key_fingerprint: "abc123".to_string(),
            capabilities: Some(Capabilities {
                file_transfer: true,
                screen_mirror: false,
                remote_control: false,
                clipboard_sync: true,
            }),
        };

        let mut buf = Vec::new();
        prost::Message::encode(&device, &mut buf).unwrap();
        
        let decoded = DeviceInfo::decode(Bytes::from(buf)).unwrap();
        assert_eq!(decoded.device_id, device.device_id);
        assert_eq!(decoded.port, device.port);
    }

    #[test]
    fn test_pairing_request_roundtrip() {
        let request = PairingRequest {
            pairing_id: "pair-456".to_string(),
            device_info: Some(DeviceInfo {
                device_id: "device-123".to_string(),
                device_name: "Test Device".to_string(),
                device_type: DeviceType::Phone as i32,
                os_version: "Android 13".to_string(),
                app_version: "1.0.0".to_string(),
                ip_address: "192.168.1.100".to_string(),
                port: 8080,
                public_key_fingerprint: "".to_string(),
                capabilities: None,
            }),
            requested_capabilities: Some(Capabilities {
                file_transfer: true,
                screen_mirror: true,
                remote_control: false,
                clipboard_sync: false,
            }),
        };

        let mut buf = Vec::new();
        prost::Message::encode(&request, &mut buf).unwrap();
        
        let decoded = PairingRequest::decode(Bytes::from(buf)).unwrap();
        assert_eq!(decoded.pairing_id, request.pairing_id);
    }

    #[test]
    fn test_touch_event_roundtrip() {
        let event = TouchEvent {
            sequence_id: 100,
            timestamp_ms: 1699999999,
            r#type: TouchType::TouchDown as i32,
            x: 100.5,
            y: 200.5,
            pressure: 0.8,
            touch_major: 10.0,
            pointer_id: 0,
        };

        let mut buf = Vec::new();
        prost::Message::encode(&event, &mut buf).unwrap();
        
        let decoded = TouchEvent::decode(Bytes::from(buf)).unwrap();
        assert_eq!(decoded.sequence_id, event.sequence_id);
        assert_eq!(decoded.x, event.x);
    }
}
