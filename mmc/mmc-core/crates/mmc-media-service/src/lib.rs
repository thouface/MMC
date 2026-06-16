//! Media service for MMC screen mirroring and remote control
//!
//! This crate provides media processing capabilities including:
//! - Video frame generation and processing
//! - Audio frame generation and processing
//! - Touch event handling and dispatch
//! - Key event handling and dispatch
//! - Screen mirroring session management

pub mod error;
pub mod video;
pub mod audio;
pub mod input;
pub mod session;

pub use error::{MediaError, Result};
pub use session::{MirroringSession, SessionConfig, SessionState, SessionStats};
pub use video::{VideoProcessor, VideoFrameGenerator};
pub use audio::{AudioProcessor, AudioFrameGenerator};
pub use input::{InputDispatcher, InputHandler, InputEvent};
