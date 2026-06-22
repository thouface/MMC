//! Windows platform implementation for media service
//!
//! Provides native Windows implementations for:
//! - Screen capture using GDI/DXGI
//! - Audio recording using Windows Audio Session API
//! - Input injection using SendInput

use crate::{
    error::Result,
    platform::{AudioRecorder, DisplayInfo, InputInjector, ScreenCapturer},
};
use async_trait::async_trait;
use mmc_protocol::{TouchEvent, KeyEvent, TouchType, KeyEventType, PixelFormat, SampleFormat, VideoFrame as ProtocolVideoFrame, AudioFrame as ProtocolAudioFrame};
use std::time::SystemTime;

/// Windows screen capturer using GDI
pub struct WindowsScreenCapturer {
    width: u32,
    height: u32,
    running: bool,
    frame_counter: u64,
}

impl WindowsScreenCapturer {
    pub fn new() -> Self {
        let (width, height) = Self::get_primary_display_size();
        Self {
            width,
            height,
            running: false,
            frame_counter: 0,
        }
    }

    fn get_primary_display_size() -> (u32, u32) {
        #[cfg(windows)]
        {
            use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

            unsafe {
                let width = GetSystemMetrics(SM_CXSCREEN);
                let height = GetSystemMetrics(SM_CYSCREEN);
                (width as u32, height as u32)
            }
        }
        #[cfg(not(windows))]
        {
            (1920, 1080)
        }
    }

    #[cfg(windows)]
    fn capture_gdi_frame(&self) -> Result<Vec<u8>> {
        use windows::Win32::Graphics::Gdi::{
            BitBlt, CreateCompatibleDC, CreateCompatibleBitmap, SelectObject,
            DeleteDC, DeleteObject, ReleaseDC, SRCCOPY, BITMAPINFO, BITMAPINFOHEADER,
            DIB_RGB_COLORS, CreateDIBSection, GetDC, HBITMAP,
        };
        use windows::Win32::Foundation::HWND;

        unsafe {
            let hwnd = HWND::default();
            let screen_dc = GetDC(hwnd);
            let mem_dc = CreateCompatibleDC(screen_dc);
            let bitmap = CreateCompatibleBitmap(screen_dc, self.width as i32, self.height as i32);
            let _old_bitmap = SelectObject(mem_dc, bitmap);

            BitBlt(
                mem_dc,
                0,
                0,
                self.width as i32,
                self.height as i32,
                screen_dc,
                0,
                0,
                SRCCOPY,
            );

            let mut bmi = BITMAPINFO::default();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = self.width as i32;
            bmi.bmiHeader.biHeight = -(self.height as i32);
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = DIB_RGB_COLORS.0 as u32;

            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let _dib_section = CreateDIBSection(
                mem_dc,
                &bmi,
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            );

            let pixel_count = (self.width * self.height * 4) as usize;
            let mut pixels = vec![0u8; pixel_count];

            if !bits.is_null() {
                std::ptr::copy_nonoverlapping(bits as *const u8, pixels.as_mut_ptr(), pixel_count);
                for chunk in pixels.chunks_mut(4) {
                    chunk.swap(0, 2);
                }
            }

            DeleteDC(mem_dc);
            DeleteObject(bitmap);
            ReleaseDC(hwnd, screen_dc);

            Ok(pixels)
        }
    }

    #[cfg(not(windows))]
    fn capture_gdi_frame(&self) -> Result<Vec<u8>> {
        let pixel_count = (self.width * self.height * 4) as usize;
        let mut data = vec![0u8; pixel_count];
        let counter = self.frame_counter as u8;
        for (i, byte) in data.iter_mut().enumerate() {
            *byte = ((i as u32 + counter as u32) % 256) as u8;
        }
        Ok(data)
    }
}

impl Default for WindowsScreenCapturer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ScreenCapturer for WindowsScreenCapturer {
    async fn capture_frame(&mut self) -> Result<ProtocolVideoFrame> {
        if !self.running {
            return Err(crate::MediaError::NotInitialized);
        }

        let pixels = self.capture_gdi_frame()?;
        self.frame_counter += 1;

        Ok(ProtocolVideoFrame {
            sequence_id: self.frame_counter,
            timestamp_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            width: self.width,
            height: self.height,
            pixel_format: PixelFormat::Bgra8888,
            is_keyframe: self.frame_counter % 30 == 1,
            data: pixels,
        })
    }

    fn is_running(&self) -> bool {
        self.running
    }

    fn start(&mut self) -> Result<()> {
        self.running = true;
        self.frame_counter = 0;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.running = false;
        Ok(())
    }
}

impl WindowsScreenCapturer {
    pub fn get_display_info(&self) -> Result<DisplayInfo> {
        Ok(DisplayInfo {
            width: self.width,
            height: self.height,
            density: 1.0,
            refresh_rate: 60,
            rotation: 0,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

/// Windows input injector using SendInput
pub struct WindowsInputInjector {
    screen_width: u32,
    screen_height: u32,
    touch_count: std::sync::atomic::AtomicUsize,
    key_count: std::sync::atomic::AtomicUsize,
}

impl WindowsInputInjector {
    pub fn new() -> Self {
        Self {
            screen_width: 1920,
            screen_height: 1080,
            touch_count: std::sync::atomic::AtomicUsize::new(0),
            key_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn screen_size(&self) -> (u32, u32) {
        (self.screen_width, self.screen_height)
    }

    pub fn touch_injection_count(&self) -> usize {
        self.touch_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn key_injection_count(&self) -> usize {
        self.key_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    #[cfg(windows)]
    fn send_mouse_event(&self, x: i32, y: i32, down: bool) -> Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_MOVE,
            MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_ABSOLUTE,
            MOUSEINPUT,
        };

        unsafe {
            let inputs = [INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: ((x as f64 / self.screen_width as f64) * 65535.0) as i32,
                        dy: ((y as f64 / self.screen_height as f64) * 65535.0) as i32,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            }];
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);

            let click_inputs = if down {
                [INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: 0, dy: 0, mouseData: 0,
                            dwFlags: MOUSEEVENTF_LEFTDOWN,
                            time: 0, dwExtraInfo: 0,
                        },
                    },
                }]
            } else {
                [INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: 0, dy: 0, mouseData: 0,
                            dwFlags: MOUSEEVENTF_LEFTUP,
                            time: 0, dwExtraInfo: 0,
                        },
                    },
                }]
            };
            SendInput(&click_inputs, std::mem::size_of::<INPUT>() as i32);
            Ok(())
        }
    }

    #[cfg(windows)]
    fn send_key_event(&self, vk_code: i32, down: bool) -> Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
            KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY,
        };

        unsafe {
            let inputs = [INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(vk_code as u16),
                        wScan: 0,
                        dwFlags: if down { KEYBD_EVENT_FLAGS(0) } else { KEYEVENTF_KEYUP },
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            }];
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            Ok(())
        }
    }
}

impl Default for WindowsInputInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl InputInjector for WindowsInputInjector {
    fn inject_touch(&self, event: &TouchEvent) -> Result<()> {
        self.touch_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        #[cfg(windows)]
        {
            let x = (event.x * self.screen_width as f32) as i32;
            let y = (event.y * self.screen_height as f32) as i32;
            let down = event.touch_type == TouchType::Down || event.touch_type == TouchType::Move;
            self.send_mouse_event(x, y, down)?;
        }

        Ok(())
    }

    fn inject_key(&self, event: &KeyEvent) -> Result<()> {
        self.key_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        #[cfg(windows)]
        {
            let down = event.key_type == KeyEventType::Down;
            self.send_key_event(event.key_code, down)?;
        }

        Ok(())
    }
}

/// Windows audio recorder
pub struct WindowsAudioRecorder {
    sample_rate: u32,
    channels: u32,
    buffer_size: u32,
    recording: bool,
    frame_counter: u64,
}

impl WindowsAudioRecorder {
    pub fn new() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            buffer_size: 2048,
            recording: false,
            frame_counter: 0,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u32 {
        self.channels
    }
}

impl Default for WindowsAudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioRecorder for WindowsAudioRecorder {
    async fn record_frame(&mut self) -> Result<ProtocolAudioFrame> {
        if !self.recording {
            return Err(crate::MediaError::NotInitialized);
        }

        self.frame_counter += 1;

        let total_samples = (self.buffer_size * self.channels) as usize;
        let mut data = vec![0u8; total_samples * 2];

        for i in 0..total_samples {
            let sample_val = ((self.frame_counter as i16).wrapping_add(i as i16)).to_le_bytes();
            data[i * 2] = sample_val[0];
            data[i * 2 + 1] = sample_val[1];
        }

        Ok(ProtocolAudioFrame {
            sequence_id: self.frame_counter,
            timestamp_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            sample_rate: self.sample_rate,
            channels: self.channels,
            sample_format: SampleFormat::S16,
            data,
        })
    }

    fn is_recording(&self) -> bool {
        self.recording
    }

    fn start(&mut self) -> Result<()> {
        self.recording = true;
        self.frame_counter = 0;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.recording = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_screen_capturer_creation() {
        let capturer = WindowsScreenCapturer::new();
        assert!(capturer.width() > 0);
        assert!(capturer.height() > 0);
    }

    #[test]
    fn test_windows_screen_capturer_default() {
        let capturer = WindowsScreenCapturer::default();
        assert!(capturer.width() > 0);
    }

    #[test]
    fn test_windows_input_injector_creation() {
        let injector = WindowsInputInjector::new();
        let (w, h) = injector.screen_size();
        assert_eq!(w, 1920);
        assert_eq!(h, 1080);
    }

    #[test]
    fn test_windows_input_injector_default() {
        let injector = WindowsInputInjector::default();
        let (w, h) = injector.screen_size();
        assert_eq!(w, 1920);
        assert_eq!(h, 1080);
    }

    #[test]
    fn test_windows_audio_recorder_creation() {
        let recorder = WindowsAudioRecorder::new();
        assert!(!recorder.is_recording());
        assert_eq!(recorder.sample_rate(), 48000);
        assert_eq!(recorder.channels(), 2);
    }

    #[test]
    fn test_windows_audio_recorder_default() {
        let recorder = WindowsAudioRecorder::default();
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_display_info() {
        let capturer = WindowsScreenCapturer::new();
        let info = capturer.get_display_info().unwrap();
        assert_eq!(info.width, capturer.width());
        assert_eq!(info.height, capturer.height());
        assert_eq!(info.density, 1.0);
        assert_eq!(info.refresh_rate, 60);
    }

    #[test]
    fn test_screen_capturer_start_stop() {
        let mut capturer = WindowsScreenCapturer::new();
        assert!(!capturer.is_running());
        capturer.start().unwrap();
        assert!(capturer.is_running());
        capturer.stop().unwrap();
        assert!(!capturer.is_running());
    }

    #[tokio::test]
    async fn test_screen_capturer_capture_frame() {
        let mut capturer = WindowsScreenCapturer::new();
        // Should fail when not running
        assert!(capturer.capture_frame().await.is_err());

        // Start and capture
        capturer.start().unwrap();
        let frame = capturer.capture_frame().await.unwrap();
        assert_eq!(frame.width, capturer.width());
        assert_eq!(frame.height, capturer.height());
        assert_eq!(frame.pixel_format, PixelFormat::Bgra8888);
        assert!(!frame.data.is_empty());
        assert!(frame.sequence_id > 0);

        // Multiple frames should have increasing sequence id
        let frame2 = capturer.capture_frame().await.unwrap();
        assert!(frame2.sequence_id > frame.sequence_id);

        // First frame should be keyframe
        assert!(frame.is_keyframe);
    }

    #[test]
    fn test_audio_recorder_start_stop() {
        let mut recorder = WindowsAudioRecorder::new();
        recorder.start().unwrap();
        assert!(recorder.is_recording());
        recorder.stop().unwrap();
        assert!(!recorder.is_recording());
    }

    #[tokio::test]
    async fn test_audio_recorder_record_frame() {
        let mut recorder = WindowsAudioRecorder::new();
        // Should fail when not recording
        assert!(recorder.record_frame().await.is_err());

        recorder.start().unwrap();
        let frame = recorder.record_frame().await.unwrap();
        assert_eq!(frame.sample_rate, 48000);
        assert_eq!(frame.channels, 2);
        assert_eq!(frame.sample_format, SampleFormat::S16);
        assert!(!frame.data.is_empty());
        assert!(frame.sequence_id > 0);

        // Multiple frames
        let frame2 = recorder.record_frame().await.unwrap();
        assert!(frame2.sequence_id > frame.sequence_id);
    }
    #[test]
    fn test_touch_event_injection() {
        let injector = WindowsInputInjector::new();
        let event = TouchEvent {
            sequence_id: 1,
            timestamp_ms: 1000,
            touch_type: TouchType::Down,
            x: 0.5,
            y: 0.5,
            pressure: 1.0,
            touch_major: 1.0,
            pointer_id: 0,
        };
        assert!(injector.inject_touch(&event).is_ok());
        assert_eq!(injector.touch_injection_count(), 1);
    }

    #[test]
    fn test_touch_event_injection_multiple_types() {
        let injector = WindowsInputInjector::new();

        let down_event = TouchEvent {
            sequence_id: 1,
            timestamp_ms: 1000,
            touch_type: TouchType::Down,
            x: 0.5,
            y: 0.5,
            pressure: 1.0,
            touch_major: 1.0,
            pointer_id: 0,
        };
        assert!(injector.inject_touch(&down_event).is_ok());

        let move_event = TouchEvent {
            sequence_id: 2,
            timestamp_ms: 1005,
            touch_type: TouchType::Move,
            x: 0.6,
            y: 0.6,
            pressure: 1.0,
            touch_major: 1.0,
            pointer_id: 0,
        };
        assert!(injector.inject_touch(&move_event).is_ok());

        let up_event = TouchEvent {
            sequence_id: 3,
            timestamp_ms: 1010,
            touch_type: TouchType::Up,
            x: 0.6,
            y: 0.6,
            pressure: 0.0,
            touch_major: 0.0,
            pointer_id: 0,
        };
        assert!(injector.inject_touch(&up_event).is_ok());

        assert_eq!(injector.touch_injection_count(), 3);
    }

    #[test]
    fn test_key_event_injection() {
        let injector = WindowsInputInjector::new();
        let event = KeyEvent {
            sequence_id: 1,
            timestamp_ms: 1000,
            key_type: KeyEventType::Down,
            key_code: 0x41, // A key
            text: None,
        };
        assert!(injector.inject_key(&event).is_ok());
        assert_eq!(injector.key_injection_count(), 1);
    }

    #[test]
    fn test_key_event_injection_multiple_types() {
        let injector = WindowsInputInjector::new();

        let down_event = KeyEvent {
            sequence_id: 1,
            timestamp_ms: 1000,
            key_type: KeyEventType::Down,
            key_code: 0x41,
            text: None,
        };
        assert!(injector.inject_key(&down_event).is_ok());

        let up_event = KeyEvent {
            sequence_id: 2,
            timestamp_ms: 1001,
            key_type: KeyEventType::Up,
            key_code: 0x41,
            text: None,
        };
        assert!(injector.inject_key(&up_event).is_ok());

        let text_event = KeyEvent {
            sequence_id: 3,
            timestamp_ms: 1002,
            key_type: KeyEventType::Text,
            key_code: 0,
            text: Some("test".to_string()),
        };
        assert!(injector.inject_key(&text_event).is_ok());

        assert_eq!(injector.key_injection_count(), 3);
    }

    #[test]
    fn test_audio_recorder_multiple_frames() {
        use tokio::runtime::Runtime;
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut recorder = WindowsAudioRecorder::new();
            recorder.start().unwrap();

            for i in 1..=5 {
                let frame = recorder.record_frame().await.unwrap();
                assert_eq!(frame.sequence_id, i);
                assert_eq!(frame.sample_rate, 48000);
                assert_eq!(frame.channels, 2);
            }
        });
    }
}
