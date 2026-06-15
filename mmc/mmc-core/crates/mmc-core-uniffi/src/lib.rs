//! MMC Core Library - Unified API for all platforms
//!
//! This crate provides a unified interface for device discovery, pairing,
//! file transfer, and remote control across Android, iOS, and PC platforms.

pub mod core;
pub mod error;
pub mod types;

pub use core::MmcCore;
pub use error::{CoreError, Result};
pub use types::*;
