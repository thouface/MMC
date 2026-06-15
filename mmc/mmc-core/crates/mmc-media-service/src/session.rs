//! Screen mirroring session management

use mmc_protocol::{PixelFormat, VideoConfig, AudioConfig, SampleFormat};
use tracing::{info, debug, warn};
use crate::error::{Result, MediaError};
use crate::video::{VideoProcessor, VideoFrameGenerator};
use crate::audio::{AudioProcessor, AudioFrameGenerator};
use crate::input::{InputDispatcher, DefaultInputHandler, InputHandler, InputEvent};

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Starting,
    Active,
    Paused,
    Stopping,
    Error,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionState::Idle => write!(f, "Idle"),
            SessionState::Starting => write!(f, "Starting"),
            SessionState::Active => write!(f, "Active"),
            SessionState::Paused => write!(f, "Paused"),
            SessionState::Stopping => write!(f, "Stopping"),
            SessionState::Error => write!(f, "Error"),
        }
    }
}

/// Session configuration
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub video_width: u32,
    pub video_height: u32,
    pub video_format: PixelFormat,
    pub frame_rate: u32,
    pub audio_sample_rate: u32,
    pub audio_channels: u32,
    pub audio_format: SampleFormat,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            video_width: 1920,
            video_height: 1080,
            video_format: PixelFormat::Rgba8888,
            frame_rate: 30,
            audio_sample_rate: 44100,
            audio_channels: 2,
            audio_format: SampleFormat::S16,
        }
    }
}

/// Mirroring session
///
/// Manages the overall screen mirroring session including video, audio, and input.
pub struct MirroringSession {
    state: SessionState,
    config: Option<SessionConfig>,
    video_processor: VideoProcessor,
    audio_processor: AudioProcessor,
    video_generator: Option<VideoFrameGenerator>,
    audio_generator: Option<AudioFrameGenerator>,
    input_dispatcher: InputDispatcher,
    input_handler: DefaultInputHandler,
    start_time: Option<std::time::Instant>,
    total_video_frames: u64,
    total_audio_frames: u64,
    total_input_events: u64,
}

impl MirroringSession {
    pub fn new() -> Self {
        Self {
            state: SessionState::Idle,
            config: None,
            video_processor: VideoProcessor::new(),
            audio_processor: AudioProcessor::new(),
            video_generator: None,
            audio_generator: None,
            input_dispatcher: InputDispatcher::new(),
            input_handler: DefaultInputHandler::new(),
            start_time: None,
            total_video_frames: 0,
            total_audio_frames: 0,
            total_input_events: 0,
        }
    }

    pub fn new_with_config(config: SessionConfig) -> Self {
        let mut session = Self::new();
        let _ = session.configure(config);
        session
    }

    pub fn configure(&mut self, config: SessionConfig) -> Result<()> {
        if self.state == SessionState::Active {
            return Err(MediaError::Session(
                "Cannot reconfigure an active session".to_string(),
            ));
        }

        if config.video_width == 0 || config.video_height == 0 {
            return Err(MediaError::InvalidConfig(
                "Video dimensions must be greater than 0".to_string(),
            ));
        }

        if config.audio_sample_rate == 0 {
            return Err(MediaError::InvalidConfig(
                "Audio sample rate must be greater than 0".to_string(),
            ));
        }

        let video_cfg = VideoConfig {
            width: config.video_width,
            height: config.video_height,
            pixel_format: config.video_format,
            frame_rate: config.frame_rate,
            codec: "raw".to_string(),
            bitrate: config.video_width * config.video_height * 4 * config.frame_rate,
        };

        self.video_processor.configure(video_cfg)?;

        let audio_cfg = AudioConfig {
            sample_rate: config.audio_sample_rate,
            channels: config.audio_channels,
            sample_format: config.audio_format,
            codec: "pcm".to_string(),
            bitrate: config.audio_sample_rate * 2 * config.audio_channels * 16,
        };

        self.audio_processor.configure(audio_cfg)?;

        self.video_generator = Some(VideoFrameGenerator::new(
            config.video_width,
            config.video_height,
            config.video_format,
        ));

        self.audio_generator = Some(AudioFrameGenerator::new(
            config.audio_sample_rate,
            config.audio_channels,
            config.audio_format,
        ));

        self.config = Some(config);
        info!("Session configured");
        Ok(())
    }

    pub fn start(&mut self) -> Result<()> {
        if self.config.is_none() {
            return Err(MediaError::NotInitialized);
        }

        if self.state == SessionState::Active {
            warn!("Session already started");
            return Ok(());
        }

        self.state = SessionState::Starting;
        self.start_time = Some(std::time::Instant::now());
        self.state = SessionState::Active;
        info!("Mirroring session started");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.state = SessionState::Stopping;
        self.start_time = None;
        self.state = SessionState::Idle;
        info!("Mirroring session stopped");
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        if self.state != SessionState::Active {
            return Err(MediaError::Session(
                "Cannot pause a non-active session".to_string(),
            ));
        }
        self.state = SessionState::Paused;
        info!("Mirroring session paused");
        Ok(())
    }

    pub fn resume(&mut self) -> Result<()> {
        if self.state != SessionState::Paused {
            return Err(MediaError::Session(
                "Cannot resume a non-paused session".to_string(),
            ));
        }
        self.state = SessionState::Active;
        info!("Mirroring session resumed");
        Ok(())
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn config(&self) -> Option<&SessionConfig> {
        self.config.as_ref()
    }

    pub fn is_active(&self) -> bool {
        self.state == SessionState::Active
    }

    pub fn generate_video_frame(&mut self) -> Result<mmc_protocol::VideoFrame> {
        if !self.is_active() {
            return Err(MediaError::Session(
                "Session is not active".to_string(),
            ));
        }

        let generator = self.video_generator.as_mut()
            .ok_or_else(|| MediaError::NotInitialized)?;

        let frame = generator.generate_frame()?;
        self.total_video_frames += 1;

        if self.total_video_frames % 60 == 0 {
            debug!("Generated {} video frames", self.total_video_frames);
        }

        Ok(frame)
    }

    pub fn generate_audio_frame(&mut self, samples_per_channel: u32) -> Result<mmc_protocol::AudioFrame> {
        if !self.is_active() {
            return Err(MediaError::Session(
                "Session is not active".to_string(),
            ));
        }

        let generator = self.audio_generator.as_mut()
            .ok_or_else(|| MediaError::NotInitialized)?;

        let frame = generator.generate_frame(samples_per_channel)?;
        self.total_audio_frames += 1;

        Ok(frame)
    }

    pub fn process_incoming_video_frame(&mut self, frame: &mmc_protocol::VideoFrame) -> Result<()> {
        if !self.is_active() {
            return Err(MediaError::Session(
                "Session is not active".to_string(),
            ));
        }
        self.video_processor.process_incoming_frame(frame)
    }

    pub fn process_incoming_audio_frame(&mut self, frame: &mmc_protocol::AudioFrame) -> Result<()> {
        if !self.is_active() {
            return Err(MediaError::Session(
                "Session is not active".to_string(),
            ));
        }
        self.audio_processor.process_incoming_frame(frame)
    }

    pub fn handle_input_event(&mut self, event: InputEvent) -> Result<()> {
        if !self.is_active() {
            return Err(MediaError::Session(
                "Session is not active".to_string(),
            ));
        }
        self.input_dispatcher.dispatch_to_handler(&event, &mut self.input_handler)?;
        self.total_input_events += 1;
        Ok(())
    }

    pub fn get_stats(&self) -> SessionStats {
        SessionStats {
            state: self.state,
            video_frames: self.total_video_frames,
            audio_frames: self.total_audio_frames,
            input_events: self.total_input_events,
            duration: self.start_time.map(|t| t.elapsed().as_secs_f32()),
        }
    }

    pub fn total_video_frames(&self) -> u64 {
        self.total_video_frames
    }

    pub fn total_audio_frames(&self) -> u64 {
        self.total_audio_frames
    }

    pub fn total_input_events(&self) -> u64 {
        self.total_input_events
    }
}

impl Default for MirroringSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Session statistics
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub state: SessionState,
    pub video_frames: u64,
    pub audio_frames: u64,
    pub input_events: u64,
    pub duration: Option<f32>,
}

impl Default for SessionStats {
    fn default() -> Self {
        Self {
            state: SessionState::Idle,
            video_frames: 0,
            audio_frames: 0,
            input_events: 0,
            duration: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mmc_protocol::TouchType;

    #[test]
    fn test_session_new() {
        let session = MirroringSession::new();
        assert_eq!(session.state(), SessionState::Idle);
        assert!(!session.is_active());
        assert!(session.config().is_none());
    }

    #[test]
    fn test_session_configure() {
        let mut session = MirroringSession::new();
        let config = SessionConfig::default();

        session.configure(config).unwrap();
        assert!(session.config().is_some());
    }

    #[test]
    fn test_session_start_stop() {
        let mut session = MirroringSession::new();
        session.configure(SessionConfig::default()).unwrap();
        session.start().unwrap();
        assert!(session.is_active());
        assert_eq!(session.state(), SessionState::Active);

        session.stop().unwrap();
        assert!(!session.is_active());
    }

    #[test]
    fn test_session_pause_resume() {
        let mut session = MirroringSession::new();
        session.configure(SessionConfig::default()).unwrap();
        session.start().unwrap();

        session.pause().unwrap();
        assert_eq!(session.state(), SessionState::Paused);

        session.resume().unwrap();
        assert_eq!(session.state(), SessionState::Active);
    }

    #[test]
    fn test_pause_without_start() {
        let mut session = MirroringSession::new();
        session.configure(SessionConfig::default()).unwrap();
        assert!(session.pause().is_err());
    }

    #[test]
    fn test_generate_video_frame() {
        let mut session = MirroringSession::new();
        session.configure(SessionConfig::default()).unwrap();
        session.start().unwrap();

        let frame = session.generate_video_frame().unwrap();
        assert_eq!(session.total_video_frames(), 1);
        assert!(frame.data.len() > 0);
    }

    #[test]
    fn test_generate_audio_frame() {
        let mut session = MirroringSession::new();
        session.configure(SessionConfig::default()).unwrap();
        session.start().unwrap();

        let frame = session.generate_audio_frame(1024).unwrap();
        assert_eq!(session.total_audio_frames(), 1);
        assert!(frame.data.len() > 0);
    }

    #[test]
    fn test_multiple_video_frames() {
        let mut session = MirroringSession::new();
        session.configure(SessionConfig::default()).unwrap();
        session.start().unwrap();

        for i in 1..=5 {
            let _frame = session.generate_video_frame().unwrap();
            assert_eq!(session.total_video_frames(), i);
        }
        assert_eq!(session.total_video_frames(), 5);
    }

    #[test]
    fn test_handle_input_event() {
        let mut session = MirroringSession::new();
        session.configure(SessionConfig::default()).unwrap();
        session.start().unwrap();

        let touch = mmc_protocol::TouchEvent {
            sequence_id: 1,
            timestamp_ms: 1000,
            touch_type: TouchType::Down,
            x: 100.0,
            y: 200.0,
            pressure: 1.0,
            touch_major: 1.0,
            pointer_id: 0,
        };
        session.handle_input_event(InputEvent::Touch(touch)).unwrap();
        assert_eq!(session.total_input_events(), 1);
    }

    #[test]
    fn test_generate_without_start() {
        let mut session = MirroringSession::new();
        session.configure(SessionConfig::default()).unwrap();
        assert!(session.generate_video_frame().is_err());
    }

    #[test]
    fn test_session_stats() {
        let mut session = MirroringSession::new();
        session.configure(SessionConfig::default()).unwrap();
        session.start().unwrap();

        for _ in 0..3 {
            let _ = session.generate_video_frame().unwrap();
            let _ = session.generate_audio_frame(512).unwrap();
        }

        let stats = session.get_stats();
        assert_eq!(stats.state, SessionState::Active);
        assert_eq!(stats.video_frames, 3);
        assert_eq!(stats.audio_frames, 3);
        assert!(stats.duration.is_some());
    }

    #[test]
    fn test_new_with_config() {
        let mut session = MirroringSession::new_with_config(SessionConfig::default());
        session.start().unwrap();
        assert!(session.is_active());
    }

    #[test]
    fn test_invalid_config() {
        let mut session = MirroringSession::new();
        let bad_config = SessionConfig {
            video_width: 0,
            video_height: 1080,
            video_format: PixelFormat::Rgba8888,
            frame_rate: 30,
            audio_sample_rate: 44100,
            audio_channels: 2,
            audio_format: SampleFormat::S16,
        };
        assert!(session.configure(bad_config).is_err());
    }

    #[test]
    fn test_invalid_audio_config() {
        let mut session = MirroringSession::new();
        let bad_config = SessionConfig {
            video_width: 1920,
            video_height: 1080,
            video_format: PixelFormat::Rgba8888,
            frame_rate: 30,
            audio_sample_rate: 0,
            audio_channels: 2,
            audio_format: SampleFormat::S16,
        };
        assert!(session.configure(bad_config).is_err());
    }

    #[test]
    fn test_double_start() {
        let mut session = MirroringSession::new();
        session.configure(SessionConfig::default()).unwrap();
        session.start().unwrap();
        session.start().unwrap(); // Should be okay - returns Ok(())
        assert!(session.is_active());
    }

    #[test]
    fn test_process_incoming_video_frame() {
        let mut session = MirroringSession::new();
        session.configure(SessionConfig::default()).unwrap();
        session.start().unwrap();

        let frame = session.generate_video_frame().unwrap();
        session.process_incoming_video_frame(&frame).unwrap();
    }

    #[test]
    fn test_process_incoming_audio_frame() {
        let mut session = MirroringSession::new();
        session.configure(SessionConfig::default()).unwrap();
        session.start().unwrap();

        let frame = session.generate_audio_frame(1024).unwrap();
        session.process_incoming_audio_frame(&frame).unwrap();
    }

    #[test]
    fn test_session_state_display() {
        assert_eq!(format!("{}", SessionState::Idle), "Idle");
        assert_eq!(format!("{}", SessionState::Active), "Active");
        assert_eq!(format!("{}", SessionState::Paused), "Paused");
    }
}
