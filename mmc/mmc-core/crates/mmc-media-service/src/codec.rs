//! Codec module - Simple video and audio compression

use crate::error::{Result, MediaError};
use mmc_protocol::{VideoFrame, AudioFrame, PixelFormat, SampleFormat};

fn pixel_format_from_i32(v: i32) -> PixelFormat {
    match v {
        1 => PixelFormat::Rgba8888,
        2 => PixelFormat::Bgra8888,
        3 => PixelFormat::Rgb565,
        4 => PixelFormat::Yuv420p,
        5 => PixelFormat::Nv12,
        _ => PixelFormat::Unknown,
    }
}

fn sample_format_from_i32(v: i32) -> SampleFormat {
    match v {
        1 => SampleFormat::U8,
        2 => SampleFormat::S16,
        3 => SampleFormat::S32,
        4 => SampleFormat::F32,
        _ => SampleFormat::Unknown,
    }
}

/// Codec trait - unified interface for encoding/decoding
pub trait Codec {
    type Input;
    type Output;

    fn name(&self) -> &str;
    fn encode(&mut self, input: &Self::Input) -> Result<Self::Output>;
    fn decode(&mut self, input: &Self::Output) -> Result<Self::Input>;
}

/// Simple wrapper for encoded data
#[derive(Debug, Clone)]
pub struct EncodedData {
    pub codec: String,
    pub original_size: usize,
    pub encoded_size: usize,
    pub data: Vec<u8>,
}

// ============================================================
// Video Codecs
// ============================================================

/// Raw video codec (no compression, just passes data through
#[derive(Debug, Clone, Default)]
pub struct RawVideoCodec;

impl RawVideoCodec {
    pub fn new() -> Self {
        Self
    }
}

impl Codec for RawVideoCodec {
    type Input = VideoFrame;
    type Output = EncodedData;

    fn name(&self) -> &str {
        "raw"
    }

    fn encode(&mut self, input: &VideoFrame) -> Result<EncodedData> {
        let mut data = Vec::with_capacity(16 + input.data.len());
        // Header: sequence_id(8) + timestamp_ms(8) + width(4) + height(4)
        // pixel_format i32(4) + is_keyframe(1)
        data.extend_from_slice(&input.sequence_id.to_le_bytes());
        data.extend_from_slice(&input.timestamp_ms.to_le_bytes());
        data.extend_from_slice(&input.width.to_le_bytes());
        data.extend_from_slice(&input.height.to_le_bytes());
        let pf = input.pixel_format as i32;
        data.extend_from_slice(&pf.to_le_bytes());
        data.push(if input.is_keyframe { 1 } else { 0 });
        data.extend_from_slice(&input.data);

        Ok(EncodedData {
            codec: "raw".to_string(),
            original_size: input.data.len(),
            encoded_size: data.len(),
            data,
        })
    }

    fn decode(&mut self, input: &EncodedData) -> Result<VideoFrame> {
        if input.data.len() < 29 {
            return Err(MediaError::FrameProcessing(
                "Invalid raw encoded data".to_string(),
            ));
        }

        let mut offset = 0;
        let sequence_id = u64::from_le_bytes(input.data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let timestamp_ms = u64::from_le_bytes(input.data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let width = u32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let height = u32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let pf = i32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let is_keyframe = input.data[offset] != 0;
        offset += 1;
        let data = input.data[offset..].to_vec();

        Ok(VideoFrame {
            sequence_id,
            timestamp_ms,
            width,
            height,
            pixel_format: pixel_format_from_i32(pf),
            is_keyframe,
            data,
        })
    }
}

/// RLE (Run-Length Encoding) video codec
#[derive(Debug, Clone, Default)]
pub struct RleVideoCodec;

impl RleVideoCodec {
    pub fn new() -> Self {
        Self
    }

    fn rle_encode(data: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        if data.is_empty() {
            return result;
        }

        let mut i = 0;
        while i < data.len() {
            let byte = data[i];
            let mut count: u16 = 1;
            while i + 1 < data.len() && data[i + 1] == byte && count < 255 {
                count += 1;
                i += 1;
            }
            result.push(count as u8);
            result.push(byte);
            i += 1;
        }
        result
    }

    fn rle_decode(data: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        let mut i = 0;
        while i + 1 < data.len() {
            let count = data[i];
            let byte = data[i + 1];
            for _ in 0..count {
                result.push(byte);
            }
            i += 2;
        }
        result
    }
}

impl Codec for RleVideoCodec {
    type Input = VideoFrame;
    type Output = EncodedData;

    fn name(&self) -> &str {
        "rle"
    }

    fn encode(&mut self, input: &VideoFrame) -> Result<EncodedData> {
        let original_size = input.data.len();
        let mut header = Vec::new();
        header.extend_from_slice(&input.sequence_id.to_le_bytes());
        header.extend_from_slice(&input.timestamp_ms.to_le_bytes());
        header.extend_from_slice(&input.width.to_le_bytes());
        header.extend_from_slice(&input.height.to_le_bytes());
        let pf = input.pixel_format as i32;
        header.extend_from_slice(&pf.to_le_bytes());
        header.push(if input.is_keyframe { 1 } else { 0 });

        let encoded = Self::rle_encode(&input.data);
        let mut data = header;
        data.extend_from_slice(&(encoded.len() as u64).to_le_bytes());
        data.extend_from_slice(&(original_size as u64).to_le_bytes());
        data.extend_from_slice(&encoded);

        Ok(EncodedData {
            codec: "rle".to_string(),
            original_size,
            encoded_size: data.len(),
            data,
        })
    }

    fn decode(&mut self, input: &EncodedData) -> Result<VideoFrame> {
        if input.data.len() < 29 {
            return Err(MediaError::FrameProcessing(
                "Invalid RLE encoded data".to_string(),
            ));
        }

        let mut offset = 0;
        let sequence_id = u64::from_le_bytes(input.data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let timestamp_ms = u64::from_le_bytes(input.data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let width = u32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let height = u32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let pf = i32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let is_keyframe = input.data[offset] != 0;
        offset += 1;

        let _enc_len = u64::from_le_bytes(input.data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let _orig_len = u64::from_le_bytes(input.data[offset..offset + 8].try_into().unwrap());
        offset += 8;

        let rle_data = &input.data[offset..];
        let data = Self::rle_decode(rle_data);

        Ok(VideoFrame {
            sequence_id,
            timestamp_ms,
            width,
            height,
            pixel_format: pixel_format_from_i32(pf),
            is_keyframe,
            data,
        })
    }
}

/// Simple Huffman-like video codec
/// Uses a fixed frequency table for byte-level compression
#[derive(Debug, Clone, Default)]
pub struct HuffmanVideoCodec;

impl HuffmanVideoCodec {
    pub fn new() -> Self {
        Self
    }

    // Simple byte-frequency based compression: count frequencies, build simple codes.
    // For simplicity, we encode as frequency table + raw indicator.
    fn huff_encode(data: &[u8]) -> Vec<u8> {
        // Count frequencies
        let mut freq = [0u32; 256];
        for &b in data {
            freq[b as usize] += 1;
        }

        // Simple encoding: build a canonical-ish encoding.
        // For simplicity, use (symbol, length_bits, bit_pattern)
        // We'll store as: [freq table 256*4 bytes][payload as bytes]
        // But to make it simple and testable, we'll use:
        // [num_symbols(2) then symbols_with_nonzero_freq then payload in simple prefix encoding
        // Actually, simplest correct implementation:
        // 1. collect non-zero freq symbols sorted by freq desc
        // 2. assign shorter codes to frequent bytes (1 bit)
        // Use bit-packing

        // Collect symbols with non-zero freq
        let mut symbols: Vec<(u32, u8)> = Vec::new();
        for (i, &f) in freq.iter().enumerate() {
            if f > 0 {
                symbols.push((f, i as u8));
            }
        }
        symbols.sort_by(|a, b| b.0.cmp(&a.0));

        // Build simple codes: symbol 0..N simple.
        // Assign codes: first symbol "0", second "10", third "110", etc. (unary-ish)

        // Serialize:
        // [count(2)][for each symbol: (symbol_byte, code_len_bits_u8, code_bytes)]
        // then [total_payload_bits(4)][payload bytes]

        // Encode payload using simple prefix codes
        // Actually, simpler approach: write out as byte pairs for simplicity.
        // Build a Vec<u8> output.

        // For simplicity, we use a simpler encoding:
        // header + run-length-ish approach.
        // [num_symbols: u16]
        // [for each symbol: byte(u8)]
        // [total bytes for counts bytes: 0..N-1]
        // Then payload = for each byte in data: use symbol index (1 byte)
        //  + run-length pair (symbol_idx, count)
        // This is RLE over symbol indices.
        // Let's instead use a cleaner simpler bit-byte-packing:
        // Store header: num_symbols u16, then symbols (u8, u32 freq)
        // payload: for each byte, emit [symbol_index_var_byte] + [count_u8] -- i.e. simplified run.

        // Build symbol table: symbol->symbol_table
        // Actually, for simplest encoding:
        // Header: num_symbols(2 bytes) then symbols in order.
        // Payload: for each run of same byte in data, emit: (index_in_table_u8, count_u8)
        // This provides lossless decompression.

        let num_symbols = symbols.len().min(256);
        let mut result = Vec::new();
        result.extend_from_slice(&(num_symbols as u16).to_le_bytes());
        for i in 0..num_symbols {
            result.push(symbols[i].1); // symbol byte
        }

        // Build symbol->index map
        let mut sym_to_idx = [0u8; 256];
        for (i, (_, sym)) in symbols.iter().enumerate() {
            sym_to_idx[*sym as usize] = i as u8;
        }

        // Encode data as (index, count) pairs
        let mut payload = Vec::new();
        if !data.is_empty() {
            let mut i = 0;
            while i < data.len() {
                let byte = data[i];
                let mut count: u8 = 1;
                while i + 1 < data.len() && data[i + 1] == byte && count < 255 {
                    count += 1;
                    i += 1;
                }
                payload.push(sym_to_idx[byte as usize]);
                payload.push(count);
                i += 1;
            }
        }
        result.extend_from_slice(&(data.len() as u64).to_le_bytes());
        result.extend_from_slice(&payload);

        result
    }

    fn huff_decode(data: &[u8]) -> Vec<u8> {
        if data.len() < 10 {
            return Vec::new();
        }
        let num_symbols = u16::from_le_bytes(data[0..2].try_into().unwrap()) as usize;
        if data.len() < 2 + num_symbols + 8 {
            return Vec::new();
        }
        let symbols = &data[2..2 + num_symbols].to_vec();
        let offset_after = 2 + num_symbols;
        let original_len = u64::from_le_bytes(data[offset_after..offset_after + 8].try_into().unwrap()) as usize;
        let payload = &data[offset_after + 8..];

        let mut result = Vec::with_capacity(original_len);
        let mut i = 0;
        while i + 1 < payload.len() {
            let sym_idx = payload[i] as usize;
            let count = payload[i + 1];
            if sym_idx < symbols.len() {
                for _ in 0..count {
                    result.push(symbols[sym_idx]);
                }
            }
            i += 2;
        }
        result
    }
}

impl Codec for HuffmanVideoCodec {
    type Input = VideoFrame;
    type Output = EncodedData;

    fn name(&self) -> &str {
        "huffman"
    }

    fn encode(&mut self, input: &VideoFrame) -> Result<EncodedData> {
        let original_size = input.data.len();
        let mut header = Vec::new();
        header.extend_from_slice(&input.sequence_id.to_le_bytes());
        header.extend_from_slice(&input.timestamp_ms.to_le_bytes());
        header.extend_from_slice(&input.width.to_le_bytes());
        header.extend_from_slice(&input.height.to_le_bytes());
        let pf = input.pixel_format as i32;
        header.extend_from_slice(&pf.to_le_bytes());
        header.push(if input.is_keyframe { 1 } else { 0 });

        let encoded = Self::huff_encode(&input.data);
        let mut data = header;
        data.extend_from_slice(&encoded);

        Ok(EncodedData {
            codec: "huffman".to_string(),
            original_size,
            encoded_size: data.len(),
            data,
        })
    }

    fn decode(&mut self, input: &EncodedData) -> Result<VideoFrame> {
        if input.data.len() < 29 {
            return Err(MediaError::FrameProcessing(
                "Invalid huffman encoded data".to_string(),
            ));
        }
        let mut offset = 0;
        let sequence_id = u64::from_le_bytes(input.data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let timestamp_ms = u64::from_le_bytes(input.data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let width = u32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let height = u32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let pf = i32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let is_keyframe = input.data[offset] != 0;
        offset += 1;

        let data = Self::huff_decode(&input.data[offset..]);

        Ok(VideoFrame {
            sequence_id,
            timestamp_ms,
            width,
            height,
            pixel_format: pixel_format_from_i32(pf),
            is_keyframe,
            data,
        })
    }
}

// ============================================================
// Audio Codecs
// ============================================================

/// PCM audio codec (no compression)
#[derive(Debug, Clone, Default)]
pub struct PcmAudioCodec;

impl PcmAudioCodec {
    pub fn new() -> Self {
        Self
    }
}

impl Codec for PcmAudioCodec {
    type Input = AudioFrame;
    type Output = EncodedData;

    fn name(&self) -> &str {
        "pcm"
    }

    fn encode(&mut self, input: &AudioFrame) -> Result<EncodedData> {
        let original_size = input.data.len();
        let mut data = Vec::new();
        data.extend_from_slice(&input.sequence_id.to_le_bytes());
        data.extend_from_slice(&input.timestamp_ms.to_le_bytes());
        data.extend_from_slice(&input.sample_rate.to_le_bytes());
        data.extend_from_slice(&input.channels.to_le_bytes());
        let sf = input.sample_format as i32;
        data.extend_from_slice(&sf.to_le_bytes());
        data.extend_from_slice(&input.data);

        Ok(EncodedData {
            codec: "pcm".to_string(),
            original_size,
            encoded_size: data.len(),
            data,
        })
    }

    fn decode(&mut self, input: &EncodedData) -> Result<AudioFrame> {
        if input.data.len() < 28 {
            return Err(MediaError::FrameProcessing(
                "Invalid PCM encoded data".to_string(),
            ));
        }

        let mut offset = 0;
        let sequence_id = u64::from_le_bytes(input.data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let timestamp_ms = u64::from_le_bytes(input.data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let sample_rate = u32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let channels = u32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let sf = i32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;

        let data = input.data[offset..].to_vec();

        Ok(AudioFrame {
            sequence_id,
            timestamp_ms,
            sample_rate,
            channels,
            sample_format: sample_format_from_i32(sf),
            data,
        })
    }
}

/// Differential audio codec (差分编码)
/// Stores first sample, then differences between consecutive samples
#[derive(Debug, Clone, Default)]
pub struct DifferentialAudioCodec;

impl DifferentialAudioCodec {
    pub fn new() -> Self {
        Self
    }

    fn diff_encode(data: &[u8]) -> Vec<u8> {
        if data.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::with_capacity(data.len());
        result.push(data[0]);
        for i in 1..data.len() {
            result.push(data[i].wrapping_sub(data[i - 1]));
        }
        result
    }

    fn diff_decode(data: &[u8]) -> Vec<u8> {
        if data.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::with_capacity(data.len());
        result.push(data[0]);
        for i in 1..data.len() {
            result.push(result[i - 1].wrapping_add(data[i]));
        }
        result
    }
}

impl Codec for DifferentialAudioCodec {
    type Input = AudioFrame;
    type Output = EncodedData;

    fn name(&self) -> &str {
        "differential"
    }

    fn encode(&mut self, input: &AudioFrame) -> Result<EncodedData> {
        let original_size = input.data.len();
        let mut header = Vec::new();
        header.extend_from_slice(&input.sequence_id.to_le_bytes());
        header.extend_from_slice(&input.timestamp_ms.to_le_bytes());
        header.extend_from_slice(&input.sample_rate.to_le_bytes());
        header.extend_from_slice(&input.channels.to_le_bytes());
        let sf = input.sample_format as i32;
        header.extend_from_slice(&sf.to_le_bytes());
        let encoded = Self::diff_encode(&input.data);
        let mut data = header;
        data.extend_from_slice(&encoded);

        Ok(EncodedData {
            codec: "differential".to_string(),
            original_size,
            encoded_size: data.len(),
            data,
        })
    }

    fn decode(&mut self, input: &EncodedData) -> Result<AudioFrame> {
        if input.data.len() < 28 {
            return Err(MediaError::FrameProcessing(
                "Invalid differential encoded data".to_string(),
            ));
        }

        let mut offset = 0;
        let sequence_id = u64::from_le_bytes(input.data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let timestamp_ms = u64::from_le_bytes(input.data[offset..offset + 8].try_into().unwrap());
        offset += 8;
        let sample_rate = u32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let channels = u32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;
        let sf = i32::from_le_bytes(input.data[offset..offset + 4].try_into().unwrap());
        offset += 4;

        let data = Self::diff_decode(&input.data[offset..]);

        Ok(AudioFrame {
            sequence_id,
            timestamp_ms,
            sample_rate,
            channels,
            sample_format: sample_format_from_i32(sf),
            data,
        })
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use mmc_protocol::{PixelFormat, SampleFormat};

    fn make_test_video_frame() -> VideoFrame {
        // Create a frame with some repeated data for compression testing
        let mut data = Vec::new();
        for i in 0..100 {
            data.push((i % 5) as u8);
            data.push(((i * 3) % 256) as u8);
        }
        // Add a run of same values
        for _ in 0..200 {
            data.push(42);
        }
        VideoFrame {
            sequence_id: 1,
            timestamp_ms: 1000,
            width: 10,
            height: 10,
            pixel_format: PixelFormat::Rgba8888,
            is_keyframe: true,
            data,
        }
    }

    fn make_test_audio_frame() -> AudioFrame {
        let mut data = Vec::new();
        for i in 0..256u32 {
            data.push((i % 256) as u8);
        }
        // Add a smooth region
        for i in 0..100u32 {
            data.push(100 + (i % 50) as u8);
        }
        AudioFrame {
            sequence_id: 1,
            timestamp_ms: 500,
            sample_rate: 44100,
            channels: 2,
            sample_format: SampleFormat::S16,
            data,
        }
    }

    #[test]
    fn test_raw_video_codec_roundtrip() {
        let mut codec = RawVideoCodec::new();
        let frame = make_test_video_frame();
        let encoded = codec.encode(&frame).unwrap();
        assert_eq!(encoded.codec, "raw");
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(decoded.sequence_id, frame.sequence_id);
        assert_eq!(decoded.timestamp_ms, frame.timestamp_ms);
        assert_eq!(decoded.width, frame.width);
        assert_eq!(decoded.height, frame.height);
        assert_eq!(decoded.pixel_format, frame.pixel_format);
        assert_eq!(decoded.is_keyframe, frame.is_keyframe);
        assert_eq!(decoded.data, frame.data);
    }

    #[test]
    fn test_rle_video_codec_roundtrip() {
        let mut codec = RleVideoCodec::new();
        let frame = make_test_video_frame();
        let encoded = codec.encode(&frame).unwrap();
        assert_eq!(encoded.codec, "rle");
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(decoded.sequence_id, frame.sequence_id);
        assert_eq!(decoded.data, frame.data);
    }

    #[test]
    fn test_huffman_video_codec_roundtrip() {
        let mut codec = HuffmanVideoCodec::new();
        let frame = make_test_video_frame();
        let encoded = codec.encode(&frame).unwrap();
        assert_eq!(encoded.codec, "huffman");
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(decoded.sequence_id, frame.sequence_id);
        assert_eq!(decoded.data, frame.data);
    }

    #[test]
    fn test_rle_internal_encode_decode() {
        let data = vec![1, 1, 1, 2, 2, 3, 3, 3, 3];
        let enc = RleVideoCodec::rle_encode(&data);
        let dec = RleVideoCodec::rle_decode(&enc);
        assert_eq!(dec, data);
        assert!(enc.len() < data.len() * 2); // should be shorter than 2x at least
    }

    #[test]
    fn test_pcm_audio_codec_roundtrip() {
        let mut codec = PcmAudioCodec::new();
        let frame = make_test_audio_frame();
        let encoded = codec.encode(&frame).unwrap();
        assert_eq!(encoded.codec, "pcm");
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(decoded.sequence_id, frame.sequence_id);
        assert_eq!(decoded.sample_rate, frame.sample_rate);
        assert_eq!(decoded.channels, frame.channels);
        assert_eq!(decoded.sample_format, frame.sample_format);
        assert_eq!(decoded.data, frame.data);
    }

    #[test]
    fn test_differential_audio_codec_roundtrip() {
        let mut codec = DifferentialAudioCodec::new();
        let frame = make_test_audio_frame();
        let encoded = codec.encode(&frame).unwrap();
        assert_eq!(encoded.codec, "differential");
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(decoded.sequence_id, frame.sequence_id);
        assert_eq!(decoded.sample_rate, frame.sample_rate);
        assert_eq!(decoded.data, frame.data);
    }

    #[test]
    fn test_diff_internal_encode_decode() {
        let data = vec![10, 11, 12, 11, 10, 9, 10];
        let enc = DifferentialAudioCodec::diff_encode(&data);
        let dec = DifferentialAudioCodec::diff_decode(&enc);
        assert_eq!(dec, data);
        // Differences should be small values
        for &d in &enc[1..] {
            let v = d as i8;
            assert!(v.abs() <= 3);
        }
    }

    #[test]
    fn test_codec_name() {
        let raw = RawVideoCodec::new();
        let rle = RleVideoCodec::new();
        let huff = HuffmanVideoCodec::new();
        let pcm = PcmAudioCodec::new();
        let diff = DifferentialAudioCodec::new();
        assert_eq!(raw.name(), "raw");
        assert_eq!(rle.name(), "rle");
        assert_eq!(huff.name(), "huffman");
        assert_eq!(pcm.name(), "pcm");
        assert_eq!(diff.name(), "differential");
    }

    #[test]
    fn test_encoded_data_creation() {
        let ed = EncodedData {
            codec: "test".to_string(),
            original_size: 100,
            encoded_size: 50,
            data: vec![1, 2, 3],
        };
        assert_eq!(ed.codec, "test");
        assert_eq!(ed.original_size, 100);
        assert_eq!(ed.encoded_size, 50);
        assert_eq!(ed.data, vec![1, 2, 3]);
    }

    #[test]
    fn test_empty_video_frame_rle() {
        let mut codec = RleVideoCodec::new();
        let frame = VideoFrame {
            sequence_id: 42,
            timestamp_ms: 9999,
            width: 0,
            height: 0,
            pixel_format: PixelFormat::Rgba8888,
            is_keyframe: false,
            data: Vec::new(),
        };
        let encoded = codec.encode(&frame).unwrap();
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(decoded.sequence_id, 42);
        assert_eq!(decoded.data.len(), 0);
    }

    #[test]
    fn test_empty_audio_frame_diff() {
        let mut codec = DifferentialAudioCodec::new();
        let frame = AudioFrame {
            sequence_id: 7,
            timestamp_ms: 123,
            sample_rate: 22050,
            channels: 1,
            sample_format: SampleFormat::U8,
            data: Vec::new(),
        };
        let encoded = codec.encode(&frame).unwrap();
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(decoded.sequence_id, 7);
        assert_eq!(decoded.data.len(), 0);
    }

    #[test]
    fn test_large_video_frame_rle_compression() {
        // Large frame with lots of repetition - should compress well
        let mut data = Vec::new();
        for _ in 0..1000 {
            data.push(5);
        }
        let mut codec = RleVideoCodec::new();
        let frame = VideoFrame {
            sequence_id: 1,
            timestamp_ms: 0,
            width: 100,
            height: 10,
            pixel_format: PixelFormat::Rgba8888,
            is_keyframe: true,
            data,
        };
        let encoded = codec.encode(&frame).unwrap();
        assert!(
            encoded.encoded_size < frame.data.len(),
            "RLE should compress repeated data: encoded={}, original={}",
            encoded.encoded_size,
            frame.data.len()
        );
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(decoded.data, frame.data);
    }
}
