//! Windows platform implementation for media service
//!
//! Provides native Windows implementations for:
//! - Screen capture using GDI/DXGI
//! - Audio recording using Windows Audio Session API
//! - Input injection using SendInput

use crate::{
    error::Result,
    platform::{AudioRecorder, DisplayInfo, InputInjector, ScreenCapturer},
    video::VideoFrame,
    audio::AudioFrame,
};
use async_trait::async_trait;
use mmc_protocol::{TouchEvent, KeyEvent, TouchType, KeyEventType};
use std::sync::Arc;

/// Windows screen capturer using GDI
pub struct WindowsScreenCapturer {
    width: u32,
    height: u32,
    running: bool,
}

impl WindowsScreenCapturer {
    pub fn new() -> Self {
        let (width, height) = Self::get_primary_display_size();
        Self { width, height, running: false }
    }

    fn get_primary_display_size() -> (u32, u32) {
        #[cfg(windows)]
        {
            use windows::Win32::Graphics::Gdi::{GetDC, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
            use windows::Win32::Foundation::HWND;

            unsafe {
                let hdc = GetDC(HWND::default());
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
            DIB_RGB_COLORS, CreateDIBSection,
        };
        use windows::Win32::Foundation::{HBITMAP, HANDLE, HWND};

        unsafe {
            let hwnd = HWND::default();
            let screen_dc = GetDC(hwnd);
            let mem_dc = CreateCompatibleDC(screen_dc);
            let bitmap = CreateCompatibleBitmap(screen_dc, self.width as i32, self.height as i32);
            let _old_bitmap = SelectObject(mem_dc, HANDLE(bitmap.0 as isize));
            
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

            // Create bitmap info for DIBSection
            let mut bmi = BITMAPINFO::default();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = self.width as i32;
            bmi.bmiHeader.biHeight = -(self.height as i32);
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = DIB_RGB_COLORS.0 as u32;

            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let _dib_section = CreateDIBSection(mem_dc, &bmi, DIB_RGB_COLORS, &mut bits, None, 0);
            
            let pixel_count = (self.width * self.height * 4) as usize;
            let mut pixels = vec![0u8; pixel_count];
            
            if !bits.is_null() {
                std::ptr::copy_nonoverlapping(bits as *const u8, pixels.as_mut_ptr(), pixel_count);
                for chunk in pixels.chunks_mut(4) {
                    chunk.swap(0, 2);
                }
            }

            DeleteDC(mem_dc);
            DeleteObject(HBITMAP(bitmap.0 as isize));
            ReleaseDC(hwnd, screen_dc);

            Ok(pixels)
        }
    }

    #[cfg(not(windows))]
    fn capture_gdi_frame(&self) -> Result<Vec<u8>> {
        Err(crate::MediaError::Capture("Not supported on non-Windows".into()).into())
    }
}

impl Default for WindowsScreenCapturer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ScreenCapturer for WindowsScreenCapturer {
    async fn capture_frame(&mut self) -> Result<VideoFrame> {
        if !self.running {
            return Err(crate::MediaError::NotInitialized);
        }
        
        let pixels = self.capture_gdi_frame()?;
        Ok(VideoFrame::new(pixels, self.width, self.height, "BGRA".to_string()))
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
}

/// Windows input injector using SendInput
pub struct WindowsInputInjector {
    screen_width: u32,
    screen_height: u32,
}

impl WindowsInputInjector {
    pub fn new() -> Self {
        Self {
            screen_width: 1920,
            screen_height: 1080,
        }
    }

    #[cfg(windows)]
    fn send_mouse_event(&self, x: i32, y: i32, down: bool) -> Result<()> {
        use windows::Win32::UI::WindowsAndMessaging::{
            SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_MOVE,
            MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_ABSOLUTE,
        };

        unsafe {
            let inputs = [INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: windows::Win32::UI::WindowsAndMessaging::MOUSEINPUT {
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
                        mi: windows::Win32::UI::WindowsAndMessaging::MOUSEINPUT {
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
                        mi: windows::Win32::UI::WindowsAndMessaging::MOUSEINPUT {
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
    fn send_key_event(&self, vk_code: u16, down: bool) -> Result<()> {
        use windows::Win32::UI::WindowsAndMessaging::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
            KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        };

        unsafe {
            let inputs = [INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: windows::Win32::UI::WindowsAndMessaging::VIRTUAL_KEY(vk_code),
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
        let x = (event.x * self.screen_width as f32) as i32;
        let y = (event.y * self.screen_height as f32) as i32;
        let down = event.touch_type == TouchType::Down || event.touch_type == TouchType::Move;
        
        #[cfg(windows)]
        {
            self.send_mouse_event(x, y, down)?;
        }
        
        Ok(())
    }

    fn inject_key(&self, event: &KeyEvent) -> Result<()> {
        let down = event.key_type == KeyEventType::Down || event.key_type == KeyEventType::Repeat;
        
        #[cfg(windows)]
        {
            self.send_key_event(event.key_code, down)?;
        }
        
        Ok(())
    }
}

/// Windows audio recorder placeholder
pub struct WindowsAudioRecorder {
    sample_rate: u32,
    channels: u16,
    buffer_size: u32,
    recording: bool,
}

impl WindowsAudioRecorder {
    pub fn new() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            buffer_size: 2048,
            recording: false,
        }
    }
}

impl Default for WindowsAudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioRecorder for WindowsAudioRecorder {
    async fn record_frame(&mut self) -> Result<AudioFrame> {
        if !self.recording {
            return Err(crate::MediaError::NotInitialized);
        }
        
        Ok(AudioFrame::new(
            vec![0u8; self.buffer_size as usize * self.channels as usize * 2],
            self.sample_rate,
            self.channels,
        ))
    }

    fn is_recording(&self) -> bool {
        self.recording
    }

    async fn start(&mut self) -> Result<()> {
        self.recording = true;
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
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
        assert!(capturer.width > 0);
        assert!(capturer.height > 0);
    }

    #[test]
    fn test_windows_input_injector_creation() {
        let injector = WindowsInputInjector::new();
        assert_eq!(injector.screen_width, 1920);
        assert_eq!(injector.screen_height, 1080);
    }

    #[test]
    fn test_windows_audio_recorder_creation() {
        let recorder = WindowsAudioRecorder::new();
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_display_info() {
        let capturer = WindowsScreenCapturer::new();
        let info = capturer.get_display_info().unwrap();
        assert_eq!(info.width, capturer.width);
        assert_eq!(info.height, capturer.height);
        assert_eq!(info.density, 1.0);
        assert_eq!(info.refresh_rate, 60);
    }

    #[tokio::test]
    async fn test_audio_recorder_start_stop() {
        let mut recorder = WindowsAudioRecorder::new();
        recorder.start().await.unwrap();
        assert!(recorder.is_recording());
        recorder.stop().await.unwrap();
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_touch_event_injection() {
        let injector = WindowsInputInjector::new();
        let event = TouchEvent {
            touch_type: TouchType::Down,
            x: 0.5,
            y: 0.5,
            pressure: 1.0,
            touch_id: 0,
        };
        assert!(injector.inject_touch(&event).is_ok());
    }

    #[test]
    fn test_key_event_injection() {
        let injector = WindowsInputInjector::new();
        let event = KeyEvent {
            key_type: KeyEventType::Down,
            key_code: 0x41, // A key
            text: None,
        };
        assert!(injector.inject_key(&event).is_ok());
    }
}
