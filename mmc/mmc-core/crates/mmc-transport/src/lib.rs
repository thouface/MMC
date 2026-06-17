//! Network transport layer for MMC.
//!
//! This crate provides TCP connection management, frame-based data transmission,
//! and heartbeat/keepalive mechanisms for device-to-device communication.

pub mod error;
pub mod connection;
pub mod frame;
pub mod heartbeat;

pub use error::{TransportError, Result};
pub use connection::{Connection, ConnectionManager, ConnectionState};
pub use frame::{FrameSender, FrameReceiver, FrameCodec};
pub use heartbeat::{HeartbeatConfig, HeartbeatMonitor};