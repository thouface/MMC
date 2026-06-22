//! Media service for MMC screen mirroring and remote control
//!
//! This crate provides media processing capabilities including:
//! - Video frame generation and processing
//! - Audio frame generation and processing
//! - Touch event handling and dispatch
//! - Key event handling and dispatch
//! - Screen mirroring session management
//! - Platform abstraction layer

pub mod error;
pub mod video;
pub mod audio;
pub mod input;
pub mod session;
pub mod codec;
pub mod platform;
pub mod platform_android;
pub mod platform_windows;

pub use error::{MediaError, Result};
pub use session::{MirroringSession, SessionConfig, SessionState, SessionStats};
pub use video::{VideoProcessor, VideoFrameGenerator};
pub use audio::{AudioProcessor, AudioFrameGenerator};
pub use input::{InputDispatcher, InputHandler, InputEvent};
pub use codec::{Codec, EncodedData, RawVideoCodec, RleVideoCodec, HuffmanVideoCodec, PcmAudioCodec, DifferentialAudioCodec};
pub use platform::{PlatformType, PlatformAdapter, ScreenCapturer, AudioRecorder, InputInjector, 
                   DisplayInfo, AudioInfo, DefaultPlatformAdapter, MockScreenCapturer, 
                   MockAudioRecorder, MockInputInjector};
pub use platform_android::{AndroidScreenCapturer, AndroidInputInjector};

pub use platform_windows::{WindowsScreenCapturer, WindowsInputInjector, WindowsAudioRecorder};
