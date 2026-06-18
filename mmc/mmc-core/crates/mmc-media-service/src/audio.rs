//! Audio processing module

use mmc_protocol::{AudioFrame, AudioConfig, SampleFormat};
use tracing::{info, debug};
use crate::error::{Result, MediaError};

/// Audio frame generator
///
/// Generates simple test audio frames for screen mirroring.
#[derive(Debug, Clone)]
pub struct AudioFrameGenerator {
    sample_rate: u32,
    channels: u32,
    sample_format: SampleFormat,
    frame_counter: u64,
    sample_counter: u64,
}

impl AudioFrameGenerator {
    pub fn new(sample_rate: u32, channels: u32, sample_format: SampleFormat) -> Self {
        Self {
            sample_rate,
            channels,
            sample_format,
            frame_counter: 0,
            sample_counter: 0,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u32 {
        self.channels
    }

    pub fn sample_format(&self) -> SampleFormat {
        self.sample_format
    }

    /// Generate a frame with the specified number of samples per channel
    pub fn generate_frame(&mut self, samples_per_channel: u32) -> Result<AudioFrame> {
        let sample_size = match self.sample_format {
            SampleFormat::U8 => 1,
            SampleFormat::S16 => 2,
            SampleFormat::S32 => 4,
            SampleFormat::F32 => 4,
            _ => 2,
        };

        let total_samples = samples_per_channel * self.channels;
        let frame_size = (total_samples * sample_size) as usize;
        let mut data = vec![0u8; frame_size];

        // Generate a simple sine wave pattern
        for i in 0..total_samples as usize {
            let t = ((self.sample_counter + i as u64) as f64) / self.sample_rate as f64;
            let freq = 440.0; // 440 Hz tone
            let sample = (2.0 * std::f64::consts::PI * freq * t).sin();

            match self.sample_format {
                SampleFormat::U8 => {
                    data[i] = ((sample * 127.0) + 128.0) as u8;
                }
                SampleFormat::S16 => {
                    let sample_i16 = (sample * 32767.0) as i16;
                    let bytes = sample_i16.to_le_bytes();
                    data[i * 2] = bytes[0];
                    data[i * 2 + 1] = bytes[1];
                }
                SampleFormat::S32 => {
                    let sample_i32 = (sample * 2147483647.0) as i32;
                    let bytes = sample_i32.to_le_bytes();
                    data[i * 4] = bytes[0];
                    data[i * 4 + 1] = bytes[1];
                    data[i * 4 + 2] = bytes[2];
                    data[i * 4 + 3] = bytes[3];
                }
                SampleFormat::F32 => {
                    let sample_f32 = sample as f32;
                    let bytes = sample_f32.to_le_bytes();
                    data[i * 4] = bytes[0];
                    data[i * 4 + 1] = bytes[1];
                    data[i * 4 + 2] = bytes[2];
                    data[i * 4 + 3] = bytes[3];
                }
                _ => {}
            }
        }

        self.frame_counter += 1;
        self.sample_counter += total_samples as u64;

        Ok(AudioFrame {
            sequence_id: self.frame_counter,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            sample_rate: self.sample_rate,
            channels: self.channels,
            sample_format: self.sample_format,
            data,
        })
    }

    pub fn generate_test_frame(sample_rate: u32, channels: u32, samples_per_channel: u32) -> Result<AudioFrame> {
        let mut generator = Self::new(sample_rate, channels, SampleFormat::S16);
        generator.generate_frame(samples_per_channel)
    }
}

/// Audio processor
///
/// Processes incoming and outgoing audio frames.
#[derive(Debug, Clone)]
pub struct AudioProcessor {
    config: Option<AudioConfig>,
    frame_count: u64,
    total_samples: u64,
}

impl AudioProcessor {
    pub fn new() -> Self {
        Self {
            config: None,
            frame_count: 0,
            total_samples: 0,
        }
    }

    pub fn configure(&mut self, config: AudioConfig) -> Result<()> {
        if config.sample_rate == 0 || config.channels == 0 {
            return Err(MediaError::InvalidConfig(
                "Sample rate and channels must be greater than 0".to_string(),
            ));
        }

        self.config = Some(config);
        info!("Audio configured: {} Hz, {} channel(s)",
            self.config.as_ref().unwrap().sample_rate,
            self.config.as_ref().unwrap().channels
        );
        Ok(())
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    pub fn config(&self) -> Option<&AudioConfig> {
        self.config.as_ref()
    }

    pub fn process_incoming_frame(&mut self, frame: &AudioFrame) -> Result<()> {
        if self.config.is_none() {
            return Err(MediaError::NotInitialized);
        }

        self.frame_count += 1;

        let sample_size = match frame.sample_format {
            SampleFormat::U8 => 1,
            SampleFormat::S16 => 2,
            SampleFormat::S32 => 4,
            SampleFormat::F32 => 4,
            _ => 2,
        };

        let samples = frame.data.len() as u64 / sample_size;
        self.total_samples += samples;

        debug!("Received audio frame #{} ({} bytes, {} samples)",
            self.frame_count,
            frame.data.len(),
            samples
        );

        Ok(())
    }

    pub fn process_outgoing_frame(&mut self, frame: &mut AudioFrame) -> Result<()> {
        if self.config.is_none() {
            return Err(MediaError::NotInitialized);
        }

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

    pub fn total_samples(&self) -> u64 {
        self.total_samples
    }
}

impl Default for AudioProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_frame_generator_new() {
        let generator = AudioFrameGenerator::new(44100, 2, SampleFormat::S16);
        assert_eq!(generator.sample_rate(), 44100);
        assert_eq!(generator.channels(), 2);
        assert_eq!(generator.sample_format(), SampleFormat::S16);
    }

    #[test]
    fn test_generate_frame_s16() {
        let mut generator = AudioFrameGenerator::new(44100, 2, SampleFormat::S16);
        let frame = generator.generate_frame(1024).unwrap();

        assert_eq!(frame.sample_rate, 44100);
        assert_eq!(frame.channels, 2);
        assert_eq!(frame.sample_format, SampleFormat::S16);
        // 1024 samples * 2 channels * 2 bytes per sample
        assert_eq!(frame.data.len(), 1024 * 2 * 2);
    }

    #[test]
    fn test_generate_frame_f32() {
        let mut generator = AudioFrameGenerator::new(48000, 1, SampleFormat::F32);
        let frame = generator.generate_frame(512).unwrap();

        assert_eq!(frame.sample_rate, 48000);
        assert_eq!(frame.channels, 1);
        assert_eq!(frame.sample_format, SampleFormat::F32);
        // 512 samples * 1 channel * 4 bytes per sample
        assert_eq!(frame.data.len(), 512 * 1 * 4);
    }

    #[test]
    fn test_generate_frame_u8() {
        let mut generator = AudioFrameGenerator::new(22050, 2, SampleFormat::U8);
        let frame = generator.generate_frame(256).unwrap();

        assert_eq!(frame.data.len(), 256 * 2 * 1);
    }

    #[test]
    fn test_generate_test_frame() {
        let frame = AudioFrameGenerator::generate_test_frame(44100, 2, 1024).unwrap();
        assert_eq!(frame.sample_rate, 44100);
        assert_eq!(frame.channels, 2);
        assert!(!frame.data.is_empty());
    }

    #[test]
    fn test_audio_processor_new() {
        let processor = AudioProcessor::new();
        assert!(!processor.is_configured());
        assert_eq!(processor.frame_count(), 0);
    }

    #[test]
    fn test_audio_processor_configure() {
        let mut processor = AudioProcessor::new();
        let config = AudioConfig {
            sample_rate: 44100,
            channels: 2,
            sample_format: SampleFormat::S16,
            codec: "pcm".to_string(),
            bitrate: 128000,
        };

        processor.configure(config).unwrap();
        assert!(processor.is_configured());
    }

    #[test]
    fn test_audio_processor_invalid_config() {
        let mut processor = AudioProcessor::new();
        let bad_config = AudioConfig {
            sample_rate: 0,
            channels: 2,
            sample_format: SampleFormat::S16,
            codec: "pcm".to_string(),
            bitrate: 128000,
        };

        assert!(processor.configure(bad_config).is_err());
    }

    #[test]
    fn test_process_incoming_audio_frame() {
        let mut processor = AudioProcessor::new();
        let config = AudioConfig {
            sample_rate: 44100,
            channels: 2,
            sample_format: SampleFormat::S16,
            codec: "pcm".to_string(),
            bitrate: 128000,
        };
        processor.configure(config).unwrap();

        let frame = AudioFrameGenerator::generate_test_frame(44100, 2, 1024).unwrap();
        processor.process_incoming_frame(&frame).unwrap();
        assert_eq!(processor.frame_count(), 1);
        assert!(processor.total_samples() > 0);
    }

    #[test]
    fn test_process_outgoing_audio_frame() {
        let mut processor = AudioProcessor::new();
        let config = AudioConfig {
            sample_rate: 48000,
            channels: 1,
            sample_format: SampleFormat::F32,
            codec: "pcm".to_string(),
            bitrate: 128000,
        };
        processor.configure(config).unwrap();

        let mut frame = AudioFrameGenerator::generate_test_frame(48000, 1, 512).unwrap();
        frame.timestamp_ms = 0;
        processor.process_outgoing_frame(&mut frame).unwrap();
        assert!(frame.timestamp_ms > 0);
    }

    #[test]
    fn test_process_frame_without_config() {
        let mut processor = AudioProcessor::new();
        let frame = AudioFrameGenerator::generate_test_frame(44100, 2, 1024).unwrap();
        assert!(processor.process_incoming_frame(&frame).is_err());
    }

    #[test]
    fn test_multiple_audio_frames_sequence() {
        let mut generator = AudioFrameGenerator::new(44100, 2, SampleFormat::S16);

        let f1 = generator.generate_frame(1024).unwrap();
        let f2 = generator.generate_frame(1024).unwrap();

        assert!(f2.sequence_id > f1.sequence_id);
    }

    #[test]
    fn test_sample_format_variants() {
        for format in [SampleFormat::U8, SampleFormat::S16,
                       SampleFormat::S32, SampleFormat::F32].iter() {
            let mut generator = AudioFrameGenerator::new(44100, 2, *format);
            let frame = generator.generate_frame(128).unwrap();
            assert_eq!(frame.sample_format, *format);
            assert!(!frame.data.is_empty());
        }
    }
}
