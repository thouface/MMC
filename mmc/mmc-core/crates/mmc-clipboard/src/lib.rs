//! Clipboard synchronization module.
//!
//! This crate provides cross-device clipboard synchronization, supporting
//! text, image (PNG), and URL content types.

pub mod error;
pub mod manager;
pub mod sync;

pub use error::{ClipboardError, Result};
pub use manager::{ClipboardManager, ClipboardEntry, ClipboardSource};
pub use sync::{ClipboardSyncer, ClipboardConfig, SyncDirection, SyncEvent};