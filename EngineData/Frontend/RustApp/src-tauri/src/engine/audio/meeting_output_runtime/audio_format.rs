use std::fs;
use std::path::Path;

use super::{
    MAX_DELIVERY_DEADLINE_MS, MAX_MEETING_OUTPUT_WAV_BYTES, MAX_OUTPUT_CHANNELS,
    MAX_OUTPUT_SAMPLE_RATE_HZ, MIN_DELIVERY_DEADLINE_MS, MIN_OUTPUT_SAMPLE_RATE_HZ,
};
use super::super::duration_ms;

pub(super) struct DecodedWav {
    pub(super) sample_rate_hz: u32,
    pub(super) channels: u16,
    pub(super) samples: Vec<f32>,
}

pub(super) fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let chunk = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| "meeting_output:wav_truncated".to_string())?;
    Ok(u16::from_le_bytes([chunk[0], chunk[1]]))
}

pub(super) fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let chunk = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| "meeting_output:wav_truncated".to_string())?;
    Ok(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
}

pub(super) fn decode_wav_bytes(bytes: &[u8]) -> Result<DecodedWav, String> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err("meeting_output:wav_invalid_riff".to_string());
    }

    let mut offset = 12usize;
    let mut format: Option<(u16, u16, u32, u16, u16)> = None;
    let mut data: Option<&[u8]> = None;

    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        let size = read_u32(bytes, offset + 4)? as usize;
        let start = offset + 8;
        let end = start
            .checked_add(size)
            .ok_or_else(|| "meeting_output:wav_chunk_overflow".to_string())?;
        if end > bytes.len() {
            return Err("meeting_output:wav_truncated".to_string());
        }

        if id == b"fmt " {
            if size < 16 {
                return Err("meeting_output:wav_fmt_invalid".to_string());
            }
            let mut audio_format = read_u16(bytes, start)?;
            let channels = read_u16(bytes, start + 2)?;
            let sample_rate = read_u32(bytes, start + 4)?;
            let block_align = read_u16(bytes, start + 12)?;
            let bits_per_sample = read_u16(bytes, start + 14)?;
            if audio_format == 0xfffe && size >= 40 {
                audio_format = read_u16(bytes, start + 24)?;
            }
            format = Some((
                audio_format,
                channels,
                sample_rate,
                block_align,
                bits_per_sample,
            ));
        } else if id == b"data" {
            data = Some(&bytes[start..end]);
        }

        offset = end + (size & 1);
    }

    let (audio_format, channels, sample_rate_hz, block_align, bits_per_sample) =
        format.ok_or_else(|| "meeting_output:wav_fmt_missing".to_string())?;
    let data = data.ok_or_else(|| "meeting_output:wav_data_missing".to_string())?;

    if channels == 0 || channels > MAX_OUTPUT_CHANNELS {
        return Err("meeting_output:wav_channels_unsupported".to_string());
    }
    if !(MIN_OUTPUT_SAMPLE_RATE_HZ..=MAX_OUTPUT_SAMPLE_RATE_HZ).contains(&sample_rate_hz) {
        return Err("meeting_output:wav_sample_rate_unsupported".to_string());
    }
    if block_align == 0 || data.len() % usize::from(block_align) != 0 {
        return Err("meeting_output:wav_frame_alignment_invalid".to_string());
    }

    let bytes_per_sample = usize::from((bits_per_sample + 7) / 8);
    if bytes_per_sample == 0 || usize::from(block_align) != bytes_per_sample * usize::from(channels)
    {
        return Err("meeting_output:wav_sample_layout_invalid".to_string());
    }

    let mut samples = Vec::with_capacity(data.len() / bytes_per_sample);
    match (audio_format, bits_per_sample) {
        (1, 8) => {
            samples.extend(data.iter().map(|value| (f32::from(*value) - 128.0) / 128.0));
        }
        (1, 16) => {
            for chunk in data.chunks_exact(2) {
                samples.push(f32::from(i16::from_le_bytes([chunk[0], chunk[1]])) / 32768.0);
            }
        }
        (1, 24) => {
            for chunk in data.chunks_exact(3) {
                let raw =
                    i32::from(chunk[0]) | (i32::from(chunk[1]) << 8) | (i32::from(chunk[2]) << 16);
                let signed = if raw & 0x0080_0000 != 0 {
                    raw | !0x00ff_ffff
                } else {
                    raw
                };
                samples.push(signed as f32 / 8_388_608.0);
            }
        }
        (1, 32) => {
            for chunk in data.chunks_exact(4) {
                let value = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                samples.push(value as f32 / 2_147_483_648.0);
            }
        }
        (3, 32) => {
            for chunk in data.chunks_exact(4) {
                let value = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                samples.push(if value.is_finite() {
                    value.clamp(-1.0, 1.0)
                } else {
                    0.0
                });
            }
        }
        _ => return Err("meeting_output:wav_encoding_unsupported".to_string()),
    }

    if samples.is_empty() {
        return Err("meeting_output:wav_empty".to_string());
    }
    Ok(DecodedWav {
        sample_rate_hz,
        channels,
        samples,
    })
}

pub(super) fn decode_wav_file(path: &Path) -> Result<DecodedWav, String> {
    let metadata = fs::metadata(path).map_err(|_| "meeting_output:wav_missing".to_string())?;
    if !metadata.is_file() {
        return Err("meeting_output:wav_missing".to_string());
    }
    if metadata.len() > MAX_MEETING_OUTPUT_WAV_BYTES {
        return Err("meeting_output:wav_too_large".to_string());
    }
    let bytes = fs::read(path).map_err(|_| "meeting_output:wav_read_failed".to_string())?;
    decode_wav_bytes(&bytes)
}

pub(super) fn prepare_output_samples(
    source: &DecodedWav,
    target_rate_hz: u32,
    target_channels: u16,
) -> Vec<f32> {
    let source_channels = usize::from(source.channels);
    let source_frames = source.samples.len() / source_channels;
    if source_frames == 0 || target_channels == 0 || target_rate_hz == 0 {
        return Vec::new();
    }

    let mut mono = Vec::with_capacity(source_frames);
    for frame in source.samples.chunks_exact(source_channels) {
        let sum = frame.iter().copied().sum::<f32>();
        mono.push(sum / source_channels as f32);
    }

    let target_frames = ((source_frames as u128 * u128::from(target_rate_hz)
        + u128::from(source.sample_rate_hz)
        - 1)
        / u128::from(source.sample_rate_hz)) as usize;
    let mut output = Vec::with_capacity(target_frames * usize::from(target_channels));
    for target_index in 0..target_frames {
        let source_position =
            target_index as f64 * source.sample_rate_hz as f64 / target_rate_hz as f64;
        let lower = source_position.floor() as usize;
        let upper = (lower + 1).min(mono.len() - 1);
        let fraction = (source_position - lower as f64) as f32;
        let value = mono[lower] + (mono[upper] - mono[lower]) * fraction;
        for _ in 0..target_channels {
            output.push(value.clamp(-1.0, 1.0));
        }
    }
    output
}

pub(super) fn delivery_deadline_ms(sample_count: usize, channels: u16, sample_rate_hz: u32) -> u64 {
    let frames = sample_count / usize::from(channels.max(1));
    u64::from(duration_ms(frames, sample_rate_hz))
        .saturating_mul(2)
        .saturating_add(2_000)
        .clamp(MIN_DELIVERY_DEADLINE_MS, MAX_DELIVERY_DEADLINE_MS)
}
