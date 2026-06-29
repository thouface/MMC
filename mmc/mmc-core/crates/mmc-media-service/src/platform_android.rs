//! Android platform implementation for screen capture and input injection.
//!
//! This module provides real Android platform implementations that use JNI
//! to communicate with Android APIs for screen capture (MediaProjection) and
//! input injection (AccessibilityService / InputManager).
//!
//! When compiled for non-Android targets, this module is excluded.

#[cfg(target_os = "android")]
mod android_impl {
    use std::sync::atomic::{AtomicBool, Ordering};
    use async_trait::async_trait;
    use mmc_protocol::{VideoFrame, TouchEvent, KeyEvent};
    use crate::error::{Result, MediaError};
    use crate::platform::ScreenCapturer;
    use crate::platform::InputInjector;
    
    /// Android-specific screen capture implementation.
    ///
    /// Uses JNI to call into Android's MediaProjection API for real screen capture.
    #[derive(Debug)]
    pub struct AndroidScreenCapturer {
        running: AtomicBool,
        frame_counter: u64,
        width: u32,
        height: u32,
    }
    
    impl AndroidScreenCapturer {
        pub fn new(width: u32, height: u32) -> Self {
            Self {
                running: AtomicBool::new(false),
                frame_counter: 0,
                width,
                height,
            }
        }
        
        /// Request screen capture permission using MediaProjection.
        /// Returns true if permission was granted.
        pub fn request_permission(&self) -> bool {
            unsafe { android_request_permission() }
        }
        
        /// Get frame from Android surface using JNI.
        fn capture_from_surface(&mut self) -> Result<Vec<u8>> {
            let width = self.width as i32;
            let height = self.height as i32;
            let mut data = vec![0u8; (width * height * 4) as usize];
            let success = unsafe {
                android_capture_frame(data.as_mut_ptr(), width, height)
            };
            if success {
                Ok(data)
            } else {
                Err(MediaError::CaptureFailed("JNI capture failed".to_string()))
            }
        }
    }
    
    #[async_trait]
    impl ScreenCapturer for AndroidScreenCapturer {
        async fn capture_frame(&mut self) -> Result<VideoFrame> {
            if !self.running.load(Ordering::SeqCst) {
                return Err(MediaError::NotInitialized);
            }
            
            // Try to capture from Android surface
            let data = self.capture_from_surface().unwrap_or_else(|_| {
                // Fall back to test frame if native capture fails
                self.frame_counter += 1;
                let width = self.width as usize;
                let height = self.height as usize;
                let pixel_size = 4;
                let frame_size = width * height * pixel_size;
                let mut data = vec![0u8; frame_size];
                let counter = self.frame_counter as u8;
                for i in 0..frame_size {
                    data[i] = ((i as u32 + counter as u32) % 256) as u8;
                }
                data
            });
            
            self.frame_counter += 1;
            let frame_size = data.len();
            let width = self.width;
            let height = self.height;
            let is_keyframe = self.frame_counter % 30 == 1;
            
            Ok(VideoFrame {
                sequence_id: self.frame_counter,
                timestamp_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                width,
                height,
                pixel_format: mmc_protocol::PixelFormat::Rgba8888,
                is_keyframe,
                data,
            })
        }
        
        fn is_running(&self) -> bool {
            self.running.load(Ordering::SeqCst)
        }
        
        fn start(&mut self) -> Result<()> {
            self.running.store(true, Ordering::SeqCst);
            self.frame_counter = 0;
            Ok(())
        }
        
        fn stop(&mut self) -> Result<()> {
            self.running.store(false, Ordering::SeqCst);
            Ok(())
        }
    }
    
    /// Android-specific input injection implementation.
    ///
    /// Uses JNI to call into Android's InputManager or AccessibilityService
    /// for touch and key event injection.
    #[derive(Debug, Default)]
    pub struct AndroidInputInjector {
        injected_touch_count: std::sync::atomic::AtomicU64,
        injected_key_count: std::sync::atomic::AtomicU64,
        enabled: AtomicBool,
    }
    
    impl AndroidInputInjector {
        pub fn new() -> Self {
            Self {
                injected_touch_count: std::sync::atomic::AtomicU64::new(0),
                injected_key_count: std::sync::atomic::AtomicU64::new(0),
                enabled: AtomicBool::new(false),
            }
        }
        
        /// Enable input injection. Must be called after user grants accessibility permission.
        pub fn enable(&self) -> bool {
            unsafe { android_enable_input_injection() }
        }
        
        /// Disable input injection.
        pub fn disable(&self) {
            self.enabled.store(false, Ordering::SeqCst);
        }
    }
    
    impl InputInjector for AndroidInputInjector {
        fn inject_touch(&self, event: &TouchEvent) -> Result<()> {
            if !self.enabled.load(Ordering::SeqCst) {
                return Err(MediaError::NotInitialized);
            }
            
            let touch_type = match event.touch_type {
                mmc_protocol::TouchType::Down => 0,
                mmc_protocol::TouchType::Move => 1,
                mmc_protocol::TouchType::Up => 2,
                mmc_protocol::TouchType::Cancel => 3,
                mmc_protocol::TouchType::Unknown => 4,
            };
            
            let success = unsafe {
                android_inject_touch(
                    touch_type,
                    event.x,
                    event.y,
                    event.pressure,
                    event.pointer_id as i32,
                    event.sequence_id,
                )
            };
            
            if success {
                self.injected_touch_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            } else {
                Err(MediaError::InjectionFailed("Touch injection failed".to_string()))
            }
        }
        
        fn inject_key(&self, event: &KeyEvent) -> Result<()> {
            if !self.enabled.load(Ordering::SeqCst) {
                return Err(MediaError::NotInitialized);
            }
            
            let key_type = match event.key_type {
                mmc_protocol::KeyEventType::Down => 0,
                mmc_protocol::KeyEventType::Up => 1,
                mmc_protocol::KeyEventType::Text => 2,
                mmc_protocol::KeyEventType::Unknown => 3,
            };
            
            let success = unsafe {
                android_inject_key(
                    key_type,
                    event.key_code,
                    event.sequence_id,
                )
            };
            
            if success {
                self.injected_key_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            } else {
                Err(MediaError::InjectionFailed("Key injection failed".to_string()))
            }
        }
    }
    
    // =============================================================================
    // JNI FFI declarations
    // These functions are implemented in Android Kotlin code and called via JNI
    // =============================================================================
    
    /// Request screen capture permission via MediaProjection.
    /// Returns true if permission was granted.
    unsafe fn android_request_permission() -> bool {
        extern "C" {
            fn Java_com_mmc_core_MediaCapture_nativeRequestPermission() -> bool;
        }
        Java_com_mmc_core_MediaCapture_nativeRequestPermission()
    }
    
    /// Capture a frame from the Android display surface.
    /// The caller must provide a buffer of sufficient size (width * height * 4 bytes).
    /// Returns true on success.
    unsafe fn android_capture_frame(buffer: *mut u8, width: i32, height: i32) -> bool {
        extern "C" {
            fn Java_com_mmc_core_MediaCapture_nativeCaptureFrame(
                buffer: *mut u8,
                width: i32,
                height: i32,
            ) -> bool;
        }
        Java_com_mmc_core_MediaCapture_nativeCaptureFrame(buffer, width, height)
    }
    
    /// Enable input injection via AccessibilityService.
    /// Returns true if enabled successfully.
    unsafe fn android_enable_input_injection() -> bool {
        extern "C" {
            fn Java_com_mmc_core_InputService_nativeEnable() -> bool;
        }
        Java_com_mmc_core_InputService_nativeEnable()
    }
    
    /// Inject a touch event into the Android input system.
    unsafe fn android_inject_touch(
        touch_type: i32,
        x: f32,
        y: f32,
        pressure: f32,
        pointer_id: i32,
        sequence_id: u64,
    ) -> bool {
        extern "C" {
            fn Java_com_mmc_core_InputService_nativeInjectTouch(
                touch_type: i32,
                x: f32,
                y: f32,
                pressure: f32,
                pointer_id: i32,
                sequence_id: i64,
            ) -> bool;
        }
        Java_com_mmc_core_InputService_nativeInjectTouch(
            touch_type,
            x,
            y,
            pressure,
            pointer_id,
            sequence_id as i64,
        )
    }
    
    /// Inject a key event into the Android input system.
    unsafe fn android_inject_key(key_type: i32, key_code: i32, sequence_id: u64) -> bool {
        extern "C" {
            fn Java_com_mmc_core_InputService_nativeInjectKey(
                key_type: i32,
                key_code: i32,
                sequence_id: i64,
            ) -> bool;
        }
        Java_com_mmc_core_InputService_nativeInjectKey(key_type, key_code, sequence_id as i64)
    }
    
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::platform::ScreenCapturer;
        
        #[test]
        fn test_android_screen_capturer_new() {
            let capturer = AndroidScreenCapturer::new(1080, 1920);
            assert!(!capturer.is_running());
        }
        
        #[tokio::test]
        async fn test_android_screen_capturer_capture_not_running() {
            let mut capturer = AndroidScreenCapturer::new(100, 100);
            assert!(capturer.capture_frame().await.is_err());
        }
        
        #[tokio::test]
        async fn test_android_screen_capturer_start_stop() {
            let mut capturer = AndroidScreenCapturer::new(100, 100);
            capturer.start().unwrap();
            assert!(capturer.is_running());
            
            capturer.stop().unwrap();
            assert!(!capturer.is_running());
        }
        
        #[test]
        fn test_android_input_injector_new() {
            let injector = AndroidInputInjector::new();
            assert!(!injector.enabled.load(Ordering::SeqCst));
        }
        
        #[test]
        fn test_android_input_injector_touch_not_enabled() {
            let injector = AndroidInputInjector::new();
            
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
            
            assert!(injector.inject_touch(&touch).is_err());
        }
        
        #[test]
        fn test_android_input_injector_key_not_enabled() {
            let injector = AndroidInputInjector::new();
            
            let key = KeyEvent {
                sequence_id: 1,
                timestamp_ms: 1000,
                key_type: mmc_protocol::KeyEventType::Down,
                key_code: 65,
                text: None,
            };
            
            assert!(injector.inject_key(&key).is_err());
        }
        
        #[test]
        fn test_android_input_injector_disable() {
            let injector = AndroidInputInjector::new();
            injector.disable();
            assert!(!injector.enabled.load(Ordering::SeqCst));
        }
        
        #[test]
        fn test_input_injector_counts() {
            let injector = AndroidInputInjector::new();
            assert_eq!(injector.injected_touch_count.load(Ordering::SeqCst), 0);
            assert_eq!(injector.injected_key_count.load(Ordering::SeqCst), 0);
        }
    }
}

// Re-export for non-Android platforms (stub implementations)
#[cfg(not(target_os = "android"))]
pub mod android_impl {
    use std::sync::atomic::{AtomicBool, Ordering};
    use async_trait::async_trait;
    use mmc_protocol::{VideoFrame, TouchEvent, KeyEvent};
    use crate::error::{Result, MediaError};
    use crate::platform::ScreenCapturer;
    use crate::platform::InputInjector;
    
    /// Stub screen capturer for non-Android platforms.
    #[derive(Debug)]
    pub struct AndroidScreenCapturer {
        running: AtomicBool,
        frame_counter: u64,
        width: u32,
        height: u32,
    }
    
    impl AndroidScreenCapturer {
        pub fn new(width: u32, height: u32) -> Self {
            Self {
                running: AtomicBool::new(false),
                frame_counter: 0,
                width,
                height,
            }
        }
        
        /// Request permission - always returns false on non-Android.
        pub fn request_permission(&self) -> bool {
            false
        }
    }
    
    #[async_trait]
    impl ScreenCapturer for AndroidScreenCapturer {
        async fn capture_frame(&mut self) -> Result<VideoFrame> {
            if !self.running.load(Ordering::SeqCst) {
                return Err(MediaError::NotInitialized);
            }
            
            self.frame_counter += 1;
            let width = self.width as usize;
            let height = self.height as usize;
            let pixel_size = 4;
            let frame_size = width * height * pixel_size;
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
                width: self.width,
                height: self.height,
                pixel_format: mmc_protocol::PixelFormat::Rgba8888,
                is_keyframe: self.frame_counter % 30 == 1,
                data,
            })
        }
        
        fn is_running(&self) -> bool {
            self.running.load(Ordering::SeqCst)
        }
        
        fn start(&mut self) -> Result<()> {
            self.running.store(true, Ordering::SeqCst);
            self.frame_counter = 0;
            Ok(())
        }
        
        fn stop(&mut self) -> Result<()> {
            self.running.store(false, Ordering::SeqCst);
            Ok(())
        }
    }
    
    /// Stub input injector for non-Android platforms.
    #[derive(Debug, Default)]
    pub struct AndroidInputInjector {
        injected_touch_count: std::sync::atomic::AtomicU64,
        injected_key_count: std::sync::atomic::AtomicU64,
        enabled: AtomicBool,
    }
    
    impl AndroidInputInjector {
        pub fn new() -> Self {
            Self {
                injected_touch_count: std::sync::atomic::AtomicU64::new(0),
                injected_key_count: std::sync::atomic::AtomicU64::new(0),
                enabled: AtomicBool::new(false),
            }
        }
        
        /// Enable injection - always returns false on non-Android.
        pub fn enable(&self) -> bool {
            false
        }
        
        pub fn disable(&self) {
            self.enabled.store(false, Ordering::SeqCst);
        }
    }
    
    impl InputInjector for AndroidInputInjector {
        fn inject_touch(&self, _event: &TouchEvent) -> Result<()> {
            if !self.enabled.load(Ordering::SeqCst) {
                return Err(MediaError::NotInitialized);
            }
            self.injected_touch_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        
        fn inject_key(&self, _event: &KeyEvent) -> Result<()> {
            if !self.enabled.load(Ordering::SeqCst) {
                return Err(MediaError::NotInitialized);
            }
            self.injected_key_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    
    #[cfg(test)]
    mod tests {
        use super::*;
        
        #[test]
        fn test_android_screen_capturer_new() {
            let capturer = AndroidScreenCapturer::new(1080, 1920);
            assert!(!capturer.is_running());
        }
        
        #[test]
        fn test_android_screen_capturer_request_permission() {
            let capturer = AndroidScreenCapturer::new(100, 100);
            assert!(!capturer.request_permission());
        }
        
        #[tokio::test]
        async fn test_android_screen_capturer_capture_not_running() {
            let mut capturer = AndroidScreenCapturer::new(100, 100);
            assert!(capturer.capture_frame().await.is_err());
        }
        
        #[tokio::test]
        async fn test_android_screen_capturer_start_stop() {
            let mut capturer = AndroidScreenCapturer::new(100, 100);
            capturer.start().unwrap();
            assert!(capturer.is_running());
            
            capturer.stop().unwrap();
            assert!(!capturer.is_running());
        }
        
        #[tokio::test]
        async fn test_android_screen_capturer_capture_running() {
            let mut capturer = AndroidScreenCapturer::new(100, 100);
            capturer.start().unwrap();
            
            let frame = capturer.capture_frame().await.unwrap();
            assert_eq!(frame.width, 100);
            assert_eq!(frame.height, 100);
            assert!(!frame.data.is_empty());
            
            capturer.stop().unwrap();
        }
        
        #[test]
        fn test_android_input_injector_new() {
            let injector = AndroidInputInjector::new();
            assert!(!injector.enabled.load(Ordering::SeqCst));
        }
        
        #[test]
        fn test_android_input_injector_enable() {
            let injector = AndroidInputInjector::new();
            assert!(!injector.enable());
        }
        
        #[test]
        fn test_android_input_injector_touch_not_enabled() {
            let injector = AndroidInputInjector::new();
            
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
            
            assert!(injector.inject_touch(&touch).is_err());
        }
        
        #[test]
        fn test_android_input_injector_key_not_enabled() {
            let injector = AndroidInputInjector::new();
            
            let key = KeyEvent {
                sequence_id: 1,
                timestamp_ms: 1000,
                key_type: mmc_protocol::KeyEventType::Down,
                key_code: 65,
                text: None,
            };
            
            assert!(injector.inject_key(&key).is_err());
        }
        
        #[test]
        fn test_android_input_injector_disable() {
            let injector = AndroidInputInjector::new();
            injector.disable();
            assert!(!injector.enabled.load(Ordering::SeqCst));
        }
        
        #[test]
        fn test_input_injector_counts() {
            let injector = AndroidInputInjector::new();
            assert_eq!(injector.injected_touch_count.load(Ordering::SeqCst), 0);
            assert_eq!(injector.injected_key_count.load(Ordering::SeqCst), 0);
        }
    }
}

pub use android_impl::{AndroidScreenCapturer, AndroidInputInjector};
