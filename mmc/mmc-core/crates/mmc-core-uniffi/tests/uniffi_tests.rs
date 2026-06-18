//! Unit tests for mmc-core-uniffi module
//!
//! These tests verify the core library types, configuration,
//! error handling, and Android JNI FFI entry points.

use mmc_core::android;
use mmc_core::types::{CoreConfig, DeviceType, TransferState, TransferProgress};
use mmc_core::error::CoreError;

// ============================================================================
// Android FFI entry point tests
// ============================================================================

#[test]
fn test_mmc_core_create_destroy() {
    let core_ptr = android::mmc_core_create();
    assert!(!core_ptr.is_null(), "mmc_core_create should return non-null pointer");

    // Destroy should not panic
    android::mmc_core_destroy(core_ptr);
}

#[test]
fn test_mmc_core_destroy_null_safe() {
    // Destroying null pointer should be safe (no panic)
    android::mmc_core_destroy(std::ptr::null_mut());
}

#[test]
fn test_mmc_core_version() {
    let version_ptr = unsafe { android::mmc_core_version() };
    assert!(!version_ptr.is_null(), "mmc_core_version should return non-null");

    let version = unsafe {
        let c_str = std::ffi::CStr::from_ptr(version_ptr);
        let s = c_str.to_string_lossy().to_string();
        android::mmc_free_string(version_ptr);
        s
    };

    assert!(!version.is_empty(), "Version string should not be empty");
    // Version should follow semver format: X.Y.Z
    let parts: Vec<&str> = version.split('.').collect();
    assert!(parts.len() >= 2, "Version should be in semver format: {}", version);
}

#[test]
fn test_mmc_free_string_null_safe() {
    // Freeing null pointer should be safe
    unsafe {
        android::mmc_free_string(std::ptr::null_mut());
    }
}

// ============================================================================
// Type conversion tests
// ============================================================================

#[test]
fn test_device_type_all_variants() {
    let types = [
        DeviceType::Unknown,
        DeviceType::Phone,
        DeviceType::Tablet,
        DeviceType::Pc,
        DeviceType::Tv,
        DeviceType::Wearable,
    ];

    for t in types {
        let debug_str = format!("{:?}", t);
        assert!(!debug_str.is_empty());
    }
}

#[test]
fn test_transfer_state_all_variants() {
    let states = [
        TransferState::Idle,
        TransferState::Preparing,
        TransferState::Transferring,
        TransferState::Paused,
        TransferState::Completed,
        TransferState::Failed,
        TransferState::Canceled,
    ];

    for s in states {
        let debug_str = format!("{:?}", s);
        assert!(!debug_str.is_empty());
    }
}

#[test]
fn test_transfer_state_equality() {
    assert_eq!(TransferState::Idle, TransferState::Idle);
    assert_eq!(TransferState::Completed, TransferState::Completed);
    assert_ne!(TransferState::Idle, TransferState::Transferring);
}

// ============================================================================
// Core configuration tests
// ============================================================================

#[test]
fn test_core_config_default() {
    let config = CoreConfig::default();

    assert!(!config.device_id.is_empty());
    assert!(!config.device_name.is_empty());
    assert_eq!(config.device_type, DeviceType::Phone);
    assert!(!config.app_version.is_empty());
    assert!(config.log_dir.is_none());
}

#[test]
fn test_core_config_from_values() {
    let config = CoreConfig {
        device_id: "test-device".to_string(),
        device_name: "Test Device".to_string(),
        device_type: DeviceType::Tablet,
        app_version: "2.0.0".to_string(),
        log_dir: Some("/tmp/mmc".to_string()),
    };

    assert_eq!(config.device_id, "test-device");
    assert_eq!(config.device_name, "Test Device");
    assert_eq!(config.device_type, DeviceType::Tablet);
    assert_eq!(config.app_version, "2.0.0");
    assert_eq!(config.log_dir, Some("/tmp/mmc".to_string()));
}

// ============================================================================
// Transfer progress tests
// ============================================================================

#[test]
fn test_transfer_progress_calculation() {
    let progress = TransferProgress {
        task_id: "task-1".to_string(),
        bytes_transferred: 500_000,
        total_bytes: 1_000_000,
        speed_bps: 100_000,
        remaining_ms: 5000,
        state: TransferState::Transferring,
        percent: 50.0,
    };

    assert_eq!(progress.bytes_transferred, 500_000);
    assert_eq!(progress.total_bytes, 1_000_000);
    assert_eq!(progress.percent, 50.0);
    assert_eq!(progress.state, TransferState::Transferring);
}

#[test]
fn test_transfer_progress_completed() {
    let progress = TransferProgress {
        task_id: "task-done".to_string(),
        bytes_transferred: 1_000_000,
        total_bytes: 1_000_000,
        speed_bps: 200_000,
        remaining_ms: 0,
        state: TransferState::Completed,
        percent: 100.0,
    };

    assert_eq!(progress.bytes_transferred, progress.total_bytes);
    assert_eq!(progress.percent, 100.0);
    assert_eq!(progress.state, TransferState::Completed);
    assert_eq!(progress.remaining_ms, 0);
}

// ============================================================================
// Error type tests
// ============================================================================

#[test]
fn test_core_error_display() {
    let errors = [
        (CoreError::NotInitialized, "Not initialized"),
        (CoreError::AlreadyInitialized, "Already initialized"),
        (CoreError::Timeout, "Timeout"),
        (CoreError::Cancelled, "Cancelled"),
    ];

    for (err, expected) in errors {
        let display = format!("{}", err);
        assert!(display.contains(expected), "Error '{}' should contain '{}'", display, expected);
    }
}

#[test]
fn test_core_error_with_message() {
    let err = CoreError::Io("file not found".to_string());
    let display = format!("{}", err);
    assert!(display.contains("file not found"));

    let err = CoreError::InitFailed("config invalid".to_string());
    let display = format!("{}", err);
    assert!(display.contains("config invalid"));
}

#[test]
fn test_core_error_from_io() {
    use std::io;
    let io_err = io::Error::new(io::ErrorKind::NotFound, "test error");
    let core_err: CoreError = io_err.into();
    match core_err {
        CoreError::Io(msg) => assert!(msg.contains("test error")),
        _ => panic!("Expected Io error"),
    }
}

// ============================================================================
// JSON serialization tests
// ============================================================================

#[test]
fn test_device_type_json() {
    use serde::{Serialize, Deserialize};
    use serde_json;

    let dt = DeviceType::Phone;
    let json = serde_json::to_string(&dt).unwrap();
    assert!(json.contains("phone"));

    let dt: DeviceType = serde_json::from_str(&json).unwrap();
    assert_eq!(dt, DeviceType::Phone);
}

#[test]
fn test_transfer_state_json() {
    use serde::{Serialize, Deserialize};
    use serde_json;

    let ts = TransferState::Transferring;
    let json = serde_json::to_string(&ts).unwrap();
    assert!(json.contains("transferring"));

    let ts: TransferState = serde_json::from_str(&json).unwrap();
    assert_eq!(ts, TransferState::Transferring);
}

// ============================================================================
// Integration: Config + Core FFI
// ============================================================================

#[test]
fn test_config_with_ffi_version() {
    let config = CoreConfig::default();
    let version_ptr = unsafe { android::mmc_core_version() };
    let version = unsafe {
        let c_str = std::ffi::CStr::from_ptr(version_ptr);
        let s = c_str.to_string_lossy().to_string();
        android::mmc_free_string(version_ptr);
        s
    };

    // Config app_version should match the FFI version
    assert_eq!(config.app_version, version);
}
