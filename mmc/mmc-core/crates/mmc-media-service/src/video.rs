//! Video processing module

use mmc_protocol::{VideoFrame, VideoConfig, PixelFormat};
use tracing::{info, debug};
use crate::error::{Result, MediaError};

/// Video frame generator
///
/// Generates simple test video frames for screen mirroring.
#[derive(Debug, Clone)]
pub struct VideoFrameGenerator {
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    frame_counter: u64,
}

impl VideoFrameGenerator {
    pub fn new(width: u32, height: u32, pixel_format: PixelFormat) -> Self {
        Self {
            width,
            height,
            pixel_format,
            frame_counter: 0,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixel_format(&self) -> PixelFormat {
        self.pixel_format
    }

    pub fn generate_frame(&mut self) -> Result<VideoFrame> {
        let pixel_size = match self.pixel_format {
            PixelFormat::Rgba8888 | PixelFormat::Bgra8888 => 4,
            PixelFormat::Rgb565 => 2,
            PixelFormat::Yuv420p | PixelFormat::Nv12 => {
                // YUV420p: Y plane + U plane + V plane
                let y_size = self.width * self.height;
                let uv_size = y_size / 4;
                let total = y_size + 2 * uv_size;
                let mut data = vec![0u8; total as usize];
                // Set a simple gradient pattern in Y plane
                for i in 0..y_size as usize {
                    data[i] = ((i + self.frame_counter as usize) % 256) as u8;
                }
                self.frame_counter += 1;
                return Ok(VideoFrame {
                    sequence_id: self.frame_counter,
                    timestamp_ms: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    width: self.width,
                    height: self.height,
                    pixel_format: self.pixel_format,
                    is_keyframe: self.frame_counter % 30 == 1,
                    data,
                });
            }
            _ => 4,
        };

        let frame_size = (self.width * self.height * pixel_size) as usize;
        let mut data = vec![0u8; frame_size];

        // Generate a simple pattern: gradient based on frame counter
        let counter = self.frame_counter as u8;
        for i in 0..frame_size {
            data[i] = ((i as u32 + counter as u32) % 256) as u8;
        }

        self.frame_counter += 1;

        Ok(VideoFrame {
            sequence_id: self.frame_counter,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            width: self.width,
            height: self.height,
            pixel_format: self.pixel_format,
            is_keyframe: self.frame_counter % 30 == 1,
            data,
        })
    }

    pub fn generate_test_frame(width: u32, height: u32) -> Result<VideoFrame> {
        let pixel_size = 4; // RGBA8888
        let frame_size = (width * height * pixel_size) as usize;
        let mut data = vec![0u8; frame_size];

        // Generate a simple test pattern
        for y in 0..height as usize {
            for x in 0..width as usize {
                let idx = (y * width as usize + x) * 4;
                data[idx] = (x % 256) as u8;        // R
                data[idx + 1] = (y % 256) as u8;    // G
                data[idx + 2] = ((x + y) % 256) as u8; // B
                data[idx + 3] = 255;                 // A
            }
        }

        Ok(VideoFrame {
            sequence_id: 1,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            width,
            height,
            pixel_format: PixelFormat::Rgba8888,
            is_keyframe: true,
            data,
        })
    }
}

/// Video processor
///
/// Processes incoming and outgoing video frames.
#[derive(Debug, Clone)]
pub struct VideoProcessor {
    config: Option<VideoConfig>,
    frame_count: u64,
    last_frame_time: Option<std::time::Instant>,
}

impl VideoProcessor {
    pub fn new() -> Self {
        Self {
            config: None,
            frame_count: 0,
            last_frame_time: None,
        }
    }

    pub fn configure(&mut self, config: VideoConfig) -> Result<()> {
        if config.width == 0 || config.height == 0 {
            return Err(MediaError::InvalidConfig(
                "Width and height must be greater than 0".to_string(),
            ));
        }

        self.config = Some(config);
        info!("Video configured: {}x{}",
            self.config.as_ref().unwrap().width,
            self.config.as_ref().unwrap().height
        );
        Ok(())
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    pub fn config(&self) -> Option<&VideoConfig> {
        self.config.as_ref()
    }

    pub fn process_incoming_frame(&mut self, frame: &VideoFrame) -> Result<()> {
        if self.config.is_none() {
            return Err(MediaError::NotInitialized);
        }

        self.frame_count += 1;
        let now = std::time::Instant::now();

        if let Some(last) = self.last_frame_time {
            let elapsed = now.duration_since(last);
            debug!("Received frame #{} ({} bytes), interval: {:?}",
                self.frame_count,
                frame.data.len(),
                elapsed
            );
        }

        self.last_frame_time = Some(now);
        Ok(())
    }

    pub fn process_outgoing_frame(&mut self, frame: &mut VideoFrame) -> Result<()> {
        if self.config.is_none() {
            return Err(MediaError::NotInitialized);
        }

        // Set timestamp if not set
        if frame.timestamp_ms == 0 {
            frame.timestamp_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
        }

        self.frame_count += 1;
        Ok(())
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }
}

impl Default for VideoProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_frame_generator_new() {
        let generator = VideoFrameGenerator::new(640, 480, PixelFormat::Rgba8888);
        assert_eq!(generator.width(), 640);
        assert_eq!(generator.height(), 480);
        assert_eq!(generator.pixel_format(), PixelFormat::Rgba8888);
    }

    #[test]
    fn test_generate_frame_rgba() {
        let mut generator = VideoFrameGenerator::new(320, 240, PixelFormat::Rgba8888);
        let frame = generator.generate_frame().unwrap();

        assert_eq!(frame.width, 320);
        assert_eq!(frame.height, 240);
        assert_eq!(frame.pixel_format, PixelFormat::Rgba8888);
        assert_eq!(frame.data.len(), (320 * 240 * 4) as usize);
    }

    #[test]
    fn test_generate_frame_yuv420p() {
        let mut generator = VideoFrameGenerator::new(640, 480, PixelFormat::Yuv420p);
        let frame = generator.generate_frame().unwrap();

        assert_eq!(frame.width, 640);
        assert_eq!(frame.height, 480);
        assert_eq!(frame.pixel_format, PixelFormat::Yuv420p);
        // YUV420p: 640*480 + 320*240 + 320*240 = 460800 bytes
        let expected_size = (640 * 480 + 2 * 320 * 240) as usize;
        assert_eq!(frame.data.len(), expected_size);
    }

    #[test]
    fn test_generate_test_frame() {
        let frame = VideoFrameGenerator::generate_test_frame(100, 100).unwrap();
        assert_eq!(frame.width, 100);
        assert_eq!(frame.height, 100);
        assert!(frame.is_keyframe);
        assert_eq!(frame.data.len(), 100 * 100 * 4);
    }

    #[test]
    fn test_video_processor_new() {
        let processor = VideoProcessor::new();
        assert!(!processor.is_configured());
        assert_eq!(processor.frame_count(), 0);
    }

    #[test]
    fn test_video_processor_configure() {
        let mut processor = VideoProcessor::new();

        let config = VideoConfig {
            width: 1920,
            height: 1080,
            pixel_format: PixelFormat::Rgba8888,
            frame_rate: 30,
            codec: "raw".to_string(),
            bitrate: 5000000,
        };

        processor.configure(config).unwrap();
        assert!(processor.is_configured());
    }

    #[test]
    fn test_video_processor_invalid_config() {
        let mut processor = VideoProcessor::new();

        let bad_config = VideoConfig {
            width: 0,
            height: 1080,
            pixel_format: PixelFormat::Rgba8888,
            frame_rate: 30,
            codec: "raw".to_string(),
            bitrate: 5000000,
        };

        assert!(processor.configure(bad_config).is_err());
    }

    #[test]
    fn test_process_incoming_frame() {
        let mut processor = VideoProcessor::new();
        let config = VideoConfig {
            width: 640,
            height: 480,
            pixel_format: PixelFormat::Rgba8888,
            frame_rate: 30,
            codec: "raw".to_string(),
            bitrate: 1000000,
        };
        processor.configure(config).unwrap();

        let frame = VideoFrameGenerator::generate_test_frame(640, 480).unwrap();
        processor.process_incoming_frame(&frame).unwrap();
        assert_eq!(processor.frame_count(), 1);
    }

    #[test]
    fn test_process_outgoing_frame() {
        let mut processor = VideoProcessor::new();
        let config = VideoConfig {
            width: 640,
            height: 480,
            pixel_format: PixelFormat::Rgba8888,
            frame_rate: 30,
            codec: "raw".to_string(),
            bitrate: 1000000,
        };
        processor.configure(config).unwrap();

        let mut frame = VideoFrameGenerator::generate_test_frame(640, 480).unwrap();
        frame.timestamp_ms = 0;
        processor.process_outgoing_frame(&mut frame).unwrap();
        assert!(frame.timestamp_ms > 0);
        assert_eq!(processor.frame_count(), 1);
    }

    #[test]
    fn test_process_frame_without_config() {
        let mut processor = VideoProcessor::new();
        let frame = VideoFrameGenerator::generate_test_frame(640, 480).unwrap();
        assert!(processor.process_incoming_frame(&frame).is_err());
    }

    #[test]
    fn test_multiple_frames_sequence() {
        let mut generator = VideoFrameGenerator::new(320, 240, PixelFormat::Rgba8888);

        let f1 = generator.generate_frame().unwrap();
        let f2 = generator.generate_frame().unwrap();

        assert!(f2.sequence_id > f1.sequence_id);
    }

    #[test]
    fn test_pixel_format_variants() {
        for format in [PixelFormat::Rgba8888, PixelFormat::Bgra8888, PixelFormat::Rgb565,
                       PixelFormat::Yuv420p, PixelFormat::Nv12].iter() {
            let mut generator = VideoFrameGenerator::new(100, 100, *format);
            let frame = generator.generate_frame().unwrap();
            assert_eq!(frame.pixel_format, *format);
            assert!(!frame.data.is_empty());
        }
    }
}
