//! Clipboard synchronization module.
//!
//! This crate provides cross-device clipboard synchronization, supporting
//! text, image (PNG), and URL content types.

pub mod error;
pub mod manager;
pub mod frame_transport;

pub use error::{ClipboardError, Result};
pub use manager::{ClipboardManager, ClipboardEntry, ClipboardSource};
pub use frame_transport::{ClipboardFrame, clipboard_data_size, CLIPBOARD_MAGIC};