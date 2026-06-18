//! Platform abstraction module.
//!
//! This module provides platform-specific abstractions for screen capture,
//! audio recording, and input injection. Different platforms (Android, iOS,
//! desktop) can implement these traits to provide platform-specific behavior.

use std::sync::Arc;
use std::fmt;
use async_trait::async_trait;
use mmc_protocol::{VideoFrame, AudioFrame, TouchEvent, KeyEvent};
use crate::error::{Result, MediaError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformType {
    Unknown,
    Android,
    Ios,
    Windows,
    Macos,
    Linux,
}

impl fmt::Display for PlatformType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformType::Unknown => write!(f, "Unknown"),
            PlatformType::Android => write!(f, "Android"),
            PlatformType::Ios => write!(f, "iOS"),
            PlatformType::Windows => write!(f, "Windows"),
            PlatformType::Macos => write!(f, "macOS"),
            PlatformType::Linux => write!(f, "Linux"),
        }
    }
}

#[async_trait]
pub trait ScreenCapturer: Send + Sync {
    async fn capture_frame(&mut self) -> Result<VideoFrame>;
    fn is_running(&self) -> bool;
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}

#[async_trait]
pub trait AudioRecorder: Send + Sync {
    async fn record_frame(&mut self) -> Result<AudioFrame>;
    fn is_recording(&self) -> bool;
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}

pub trait InputInjector: Send + Sync {
    fn inject_touch(&self, event: &TouchEvent) -> Result<()>;
    fn inject_key(&self, event: &KeyEvent) -> Result<()>;
}

pub trait PlatformAdapter: Send + Sync {
    fn platform_type(&self) -> PlatformType;
    
    fn screen_capturer(&self) -> Option<Arc<dyn ScreenCapturer>>;
    
    fn audio_recorder(&self) -> Option<Arc<dyn AudioRecorder>>;
    
    fn input_injector(&self) -> Option<Arc<dyn InputInjector>>;
    
    fn get_display_info(&self) -> Result<DisplayInfo>;
    
    fn get_audio_info(&self) -> Result<AudioInfo>;
}

#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub width: u32,
    pub height: u32,
    pub density: f32,
    pub refresh_rate: u32,
    pub rotation: u32,
}

#[derive(Debug, Clone)]
pub struct AudioInfo {
    pub sample_rate: u32,
    pub channels: u32,
    pub buffer_size: u32,
}

#[derive(Debug, Default)]
pub struct MockScreenCapturer {
    running: bool,
    frame_counter: u64,
}

impl MockScreenCapturer {
    pub fn new() -> Self {
        Self {
            running: false,
            frame_counter: 0,
        }
    }
}

#[async_trait]
impl ScreenCapturer for MockScreenCapturer {
    async fn capture_frame(&mut self) -> Result<VideoFrame> {
        if !self.running {
            return Err(MediaError::NotInitialized);
        }
        
        self.frame_counter += 1;
        
        let width = 1080;
        let height = 1920;
        let pixel_size = 4; // RGBA8888
        let frame_size = (width * height * pixel_size) as usize;
        let mut data = vec![0u8; frame_size];
        
        let counter = self.frame_counter as u8;
        for i in 0..frame_size {
            data[i] = ((i as u32 + counter as u32) % 256) as u8;
        }
        
        Ok(VideoFrame {
            sequence_id: self.frame_counter,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            width,
            height,
            pixel_format: mmc_protocol::PixelFormat::Rgba8888,
            is_keyframe: self.frame_counter % 30 == 1,
            data,
        })
    }
    
    fn is_running(&self) -> bool {
        self.running
    }
    
    fn start(&mut self) -> Result<()> {
        self.running = true;
        Ok(())
    }
    
    fn stop(&mut self) -> Result<()> {
        self.running = false;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct MockAudioRecorder {
    recording: bool,
    frame_counter: u64,
    sample_counter: u64,
}

impl MockAudioRecorder {
    pub fn new() -> Self {
        Self {
            recording: false,
            frame_counter: 0,
            sample_counter: 0,
        }
    }
}

#[async_trait]
impl AudioRecorder for MockAudioRecorder {
    async fn record_frame(&mut self) -> Result<AudioFrame> {
        if !self.recording {
            return Err(MediaError::NotInitialized);
        }
        
        self.frame_counter += 1;
        
        let sample_rate = 44100;
        let channels = 2;
        let samples_per_channel = 1024;
        let total_samples = samples_per_channel * channels;
        let sample_size = 2; // S16
        let frame_size = (total_samples * sample_size) as usize;
        let mut data = vec![0u8; frame_size];
        
        for i in 0..total_samples as usize {
            let t = ((self.sample_counter + i as u64) as f64) / sample_rate as f64;
            let freq = 440.0;
            let sample = (2.0 * std::f64::consts::PI * freq * t).sin();
            let sample_i16 = (sample * 32767.0) as i16;
            let bytes = sample_i16.to_le_bytes();
            data[i * 2] = bytes[0];
            data[i * 2 + 1] = bytes[1];
        }
        
        self.sample_counter += total_samples as u64;
        
        Ok(AudioFrame {
            sequence_id: self.frame_counter,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            sample_rate,
            channels,
            sample_format: mmc_protocol::SampleFormat::S16,
            data,
        })
    }
    
    fn is_recording(&self) -> bool {
        self.recording
    }
    
    fn start(&mut self) -> Result<()> {
        self.recording = true;
        Ok(())
    }
    
    fn stop(&mut self) -> Result<()> {
        self.recording = false;
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct MockInputInjector;

impl InputInjector for MockInputInjector {
    fn inject_touch(&self, _event: &TouchEvent) -> Result<()> {
        Ok(())
    }
    
    fn inject_key(&self, _event: &KeyEvent) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct DefaultPlatformAdapter {
    platform_type: PlatformType,
    screen_capturer: Option<Arc<MockScreenCapturer>>,
    audio_recorder: Option<Arc<MockAudioRecorder>>,
    input_injector: Option<Arc<MockInputInjector>>,
}

impl DefaultPlatformAdapter {
    pub fn new(platform_type: PlatformType) -> Self {
        Self {
            platform_type,
            screen_capturer: Some(Arc::new(MockScreenCapturer::new())),
            audio_recorder: Some(Arc::new(MockAudioRecorder::new())),
            input_injector: Some(Arc::new(MockInputInjector)),
        }
    }
    
    pub fn detect_platform() -> PlatformType {
        #[cfg(target_os = "android")]
        {
            PlatformType::Android
        }
        #[cfg(target_os = "ios")]
        {
            PlatformType::Ios
        }
        #[cfg(target_os = "windows")]
        {
            PlatformType::Windows
        }
        #[cfg(target_os = "macos")]
        {
            PlatformType::Macos
        }
        #[cfg(target_os = "linux")]
        {
            PlatformType::Linux
        }
        #[cfg(not(any(target_os = "android", target_os = "ios", target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            PlatformType::Unknown
        }
    }
    
    /// Get platform-specific default display info
    pub fn default_display_info(platform: PlatformType) -> DisplayInfo {
        match platform {
            PlatformType::Android => DisplayInfo {
                width: 1080,
                height: 1920,
                density: 2.0,
                refresh_rate: 60,
                rotation: 0,
            },
            PlatformType::Ios => DisplayInfo {
                width: 1170,
                height: 2532,
                density: 3.0,
                refresh_rate: 120,
                rotation: 0,
            },
            PlatformType::Windows => DisplayInfo {
                width: 1920,
                height: 1080,
                density: 1.0,
                refresh_rate: 60,
                rotation: 0,
            },
            PlatformType::Macos => DisplayInfo {
                width: 2560,
                height: 1600,
                density: 2.0,
                refresh_rate: 60,
                rotation: 0,
            },
            PlatformType::Linux => DisplayInfo {
                width: 1920,
                height: 1080,
                density: 1.0,
                refresh_rate: 60,
                rotation: 0,
            },
            PlatformType::Unknown => DisplayInfo {
                width: 1920,
                height: 1080,
                density: 1.0,
                refresh_rate: 60,
                rotation: 0,
            },
        }
    }
    
    /// Get platform-specific default audio info
    pub fn default_audio_info(platform: PlatformType) -> AudioInfo {
        match platform {
            PlatformType::Android | PlatformType::Ios | PlatformType::Unknown => AudioInfo {
                sample_rate: 44100,
                channels: 2,
                buffer_size: 1024,
            },
            PlatformType::Windows | PlatformType::Macos | PlatformType::Linux => AudioInfo {
                sample_rate: 48000,
                channels: 2,
                buffer_size: 2048,
            },
        }
    }
    
    pub fn create_default() -> Self {
        Self::new(Self::detect_platform())
    }
}

impl PlatformAdapter for DefaultPlatformAdapter {
    fn platform_type(&self) -> PlatformType {
        self.platform_type
    }
    
    fn screen_capturer(&self) -> Option<Arc<dyn ScreenCapturer>> {
        self.screen_capturer.as_ref().map(|c| Arc::clone(c) as Arc<dyn ScreenCapturer>)
    }
    
    fn audio_recorder(&self) -> Option<Arc<dyn AudioRecorder>> {
        self.audio_recorder.as_ref().map(|r| Arc::clone(r) as Arc<dyn AudioRecorder>)
    }
    
    fn input_injector(&self) -> Option<Arc<dyn InputInjector>> {
        self.input_injector.as_ref().map(|i| Arc::clone(i) as Arc<dyn InputInjector>)
    }
    
    fn get_display_info(&self) -> Result<DisplayInfo> {
        Ok(Self::default_display_info(self.platform_type))
    }
    
    fn get_audio_info(&self) -> Result<AudioInfo> {
        Ok(Self::default_audio_info(self.platform_type))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio;
    
    #[test]
    fn test_platform_type_display() {
        assert_eq!(PlatformType::Android.to_string(), "Android");
        assert_eq!(PlatformType::Ios.to_string(), "iOS");
        assert_eq!(PlatformType::Windows.to_string(), "Windows");
        assert_eq!(PlatformType::Macos.to_string(), "macOS");
        assert_eq!(PlatformType::Linux.to_string(), "Linux");
        assert_eq!(PlatformType::Unknown.to_string(), "Unknown");
    }
    
    #[test]
    fn test_default_platform_adapter_new() {
        let adapter = DefaultPlatformAdapter::new(PlatformType::Android);
        assert_eq!(adapter.platform_type(), PlatformType::Android);
        assert!(adapter.screen_capturer().is_some());
        assert!(adapter.audio_recorder().is_some());
        assert!(adapter.input_injector().is_some());
    }
    
    #[test]
    fn test_display_info() {
        let adapter = DefaultPlatformAdapter::new(PlatformType::Android);
        let info = adapter.get_display_info().unwrap();
        assert_eq!(info.width, 1080);
        assert_eq!(info.height, 1920);
        assert_eq!(info.density, 2.0);
        assert_eq!(info.refresh_rate, 60);
        assert_eq!(info.rotation, 0);
    }
    
    #[test]
    fn test_audio_info() {
        let adapter = DefaultPlatformAdapter::new(PlatformType::Android);
        let info = adapter.get_audio_info().unwrap();
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.channels, 2);
        assert_eq!(info.buffer_size, 1024);
    }
    
    #[tokio::test]
    async fn test_mock_screen_capturer() {
        let mut capturer = MockScreenCapturer::new();
        assert!(!capturer.is_running());
        
        capturer.start().unwrap();
        assert!(capturer.is_running());
        
        let frame = capturer.capture_frame().await.unwrap();
        assert_eq!(frame.width, 1080);
        assert_eq!(frame.height, 1920);
        assert!(!frame.data.is_empty());
        
        capturer.stop().unwrap();
        assert!(!capturer.is_running());
        
        assert!(capturer.capture_frame().await.is_err());
    }
    
    #[tokio::test]
    async fn test_mock_audio_recorder() {
        let mut recorder = MockAudioRecorder::new();
        assert!(!recorder.is_recording());
        
        recorder.start().unwrap();
        assert!(recorder.is_recording());
        
        let frame = recorder.record_frame().await.unwrap();
        assert_eq!(frame.sample_rate, 44100);
        assert_eq!(frame.channels, 2);
        assert!(!frame.data.is_empty());
        
        recorder.stop().unwrap();
        assert!(!recorder.is_recording());
        
        assert!(recorder.record_frame().await.is_err());
    }
    
    #[test]
    fn test_mock_input_injector() {
        let injector = MockInputInjector;
        
        let touch = TouchEvent {
            sequence_id: 1,
            timestamp_ms: 1000,
            touch_type: mmc_protocol::TouchType::Down,
            x: 100.0,
            y: 200.0,
            pressure: 1.0,
            touch_major: 1.0,
            pointer_id: 0,
        };
        assert!(injector.inject_touch(&touch).is_ok());
        
        let key = KeyEvent {
            sequence_id: 1,
            timestamp_ms: 1000,
            key_type: mmc_protocol::KeyEventType::Down,
            key_code: 65,
            text: None,
        };
        assert!(injector.inject_key(&key).is_ok());
    }
    
    #[test]
    fn test_screen_capturer_arc() {
        let _adapter = DefaultPlatformAdapter::new(PlatformType::Android);
        let mut capturer = MockScreenCapturer::new();
        capturer.start().unwrap();
        assert!(capturer.is_running());
    }
    
    #[test]
    fn test_audio_recorder_arc() {
        let _adapter = DefaultPlatformAdapter::new(PlatformType::Android);
        let mut recorder = MockAudioRecorder::new();
        recorder.start().unwrap();
        assert!(recorder.is_recording());
    }
    
    #[test]
    fn test_detect_platform() {
        let platform = DefaultPlatformAdapter::detect_platform();
        match platform {
            PlatformType::Android | PlatformType::Ios | PlatformType::Windows | 
            PlatformType::Macos | PlatformType::Linux | PlatformType::Unknown => {
                // All valid
            }
        }
    }
    
    #[test]
    fn test_create_default() {
        let adapter = DefaultPlatformAdapter::create_default();
        assert!(adapter.screen_capturer().is_some());
        assert!(adapter.audio_recorder().is_some());
        assert!(adapter.input_injector().is_some());
    }
    
    #[test]
    fn test_android_platform_adapter() {
        let adapter = DefaultPlatformAdapter::new(PlatformType::Android);
        assert_eq!(adapter.platform_type(), PlatformType::Android);
        
        let display = adapter.get_display_info().unwrap();
        assert_eq!(display.width, 1080);
        assert_eq!(display.height, 1920);
        assert_eq!(display.density, 2.0);
        
        let audio = adapter.get_audio_info().unwrap();
        assert_eq!(audio.sample_rate, 44100);
    }
    
    #[test]
    fn test_ios_platform_adapter() {
        let adapter = DefaultPlatformAdapter::new(PlatformType::Ios);
        assert_eq!(adapter.platform_type(), PlatformType::Ios);
        
        let display = adapter.get_display_info().unwrap();
        assert_eq!(display.width, 1170);
        assert_eq!(display.height, 2532);
        assert_eq!(display.density, 3.0);
        assert_eq!(display.refresh_rate, 120);
        
        let audio = adapter.get_audio_info().unwrap();
        assert_eq!(audio.sample_rate, 44100);
    }
    
    #[test]
    fn test_windows_platform_adapter() {
        let adapter = DefaultPlatformAdapter::new(PlatformType::Windows);
        assert_eq!(adapter.platform_type(), PlatformType::Windows);
        
        let display = adapter.get_display_info().unwrap();
        assert_eq!(display.width, 1920);
        assert_eq!(display.height, 1080);
        assert_eq!(display.density, 1.0);
        
        let audio = adapter.get_audio_info().unwrap();
        assert_eq!(audio.sample_rate, 48000);
        assert_eq!(audio.buffer_size, 2048);
    }
    
    #[test]
    fn test_macos_platform_adapter() {
        let adapter = DefaultPlatformAdapter::new(PlatformType::Macos);
        assert_eq!(adapter.platform_type(), PlatformType::Macos);
        
        let display = adapter.get_display_info().unwrap();
        assert_eq!(display.width, 2560);
        assert_eq!(display.height, 1600);
        assert_eq!(display.density, 2.0);
        
        let audio = adapter.get_audio_info().unwrap();
        assert_eq!(audio.sample_rate, 48000);
    }
    
    #[test]
    fn test_linux_platform_adapter() {
        let adapter = DefaultPlatformAdapter::new(PlatformType::Linux);
        assert_eq!(adapter.platform_type(), PlatformType::Linux);
        
        let display = adapter.get_display_info().unwrap();
        assert_eq!(display.width, 1920);
        assert_eq!(display.height, 1080);
        assert_eq!(display.density, 1.0);
        
        let audio = adapter.get_audio_info().unwrap();
        assert_eq!(audio.sample_rate, 48000);
    }
    
    #[test]
    fn test_platform_default_display_info() {
        // Test all platforms have default display info
        for platform in [
            PlatformType::Android,
            PlatformType::Ios,
            PlatformType::Windows,
            PlatformType::Macos,
            PlatformType::Linux,
            PlatformType::Unknown,
        ] {
            let info = DefaultPlatformAdapter::default_display_info(platform);
            assert!(info.width > 0);
            assert!(info.height > 0);
            assert!(info.density > 0.0);
            assert!(info.refresh_rate > 0);
        }
    }
    
    #[test]
    fn test_platform_default_audio_info() {
        // Test all platforms have default audio info
        for platform in [
            PlatformType::Android,
            PlatformType::Ios,
            PlatformType::Windows,
            PlatformType::Macos,
            PlatformType::Linux,
            PlatformType::Unknown,
        ] {
            let info = DefaultPlatformAdapter::default_audio_info(platform);
            assert!(info.sample_rate > 0);
            assert!(info.channels > 0);
            assert!(info.buffer_size > 0);
        }
    }
    
    #[test]
    fn test_all_platform_types_have_adapters() {
        let platforms = [
            PlatformType::Android,
            PlatformType::Ios,
            PlatformType::Windows,
            PlatformType::Macos,
            PlatformType::Linux,
            PlatformType::Unknown,
        ];
        
        for platform in platforms {
            let adapter = DefaultPlatformAdapter::new(platform);
            assert!(adapter.screen_capturer().is_some(), "Platform {:?} should have screen capturer", platform);
            assert!(adapter.audio_recorder().is_some(), "Platform {:?} should have audio recorder", platform);
            assert!(adapter.input_injector().is_some(), "Platform {:?} should have input injector", platform);
        }
    }
}
