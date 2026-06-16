//! MMC Core Library - Unified API for all platforms
//!
//! This crate provides a unified interface for device discovery, pairing,
//! file transfer, and remote control across Android, iOS, and PC platforms.
//!
//! ## Architecture
//!
//! The core logic is implemented in pure Rust. For cross-platform bindings,
//! UniFFI can be used to generate Android (JNI) and iOS (Swift) bindings from
//! the Rust source.
//!
//! ## Usage
//!
//! ```rust
//! use mmc_core_uniffi::MmcCore;
//! use mmc_core_uniffi::types::{CoreConfig, DeviceType};
//!
//! let core = MmcCore::new();
//! let config = CoreConfig {
//!     device_id: "my-device".to_string(),
//!     device_name: "My Device".to_string(),
//!     device_type: DeviceType::Phone,
//!     app_version: "1.0.0".to_string(),
//!     log_dir: None,
//! };
//! core.init(config);
//! ```

pub mod core;
pub mod error;
pub mod types;

pub use core::MmcCore;
pub use error::{CoreError, Result};
pub use types::*;
