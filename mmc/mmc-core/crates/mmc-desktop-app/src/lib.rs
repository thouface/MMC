//! MMC Desktop Application
//!
//! This crate provides a desktop application entry point for MMC,
//! supporting Windows, macOS, and Linux platforms.
//!
//! ## Features
//!
//! - Device discovery and pairing
//! - File transfer
//! - Clipboard synchronization
//! - Screen mirroring (mock/stub for now)
//! - Remote control (mock/stub for now)

pub mod clipboard;
pub mod commands;
pub mod error;
pub mod state;

pub use error::{DesktopError, Result};
pub use state::{AppState, PairedDevice, DesktopTransferTask, DesktopMirrorStats};

/// Get the current platform type
pub fn get_platform_type() -> mmc_media_service::platform::PlatformType {
    mmc_media_service::platform::DefaultPlatformAdapter::detect_platform()
}

/// Re-export platform types for convenience
pub use mmc_media_service::platform::{PlatformType, PlatformAdapter, ScreenCapturer, AudioRecorder, InputInjector};