pub(crate) mod device_selection;
pub mod evidence;
pub mod finalized_utterance;
pub mod guided_take;
pub mod input;
pub mod live_audio_buffer;
pub mod live_capture;
pub mod live_segment_writer;
pub mod meeting_output;
pub mod original_voice_pcm;
pub mod meeting_sound_capture;
// Deterministic handoff prototype: production callback remains unchanged until
// offline Rust execution and matched device tests prove the new boundary.
#[cfg(test)]
mod pcm_handoff;
#[cfg(test)]
mod pcm_handoff_replay;
#[cfg(test)]
mod audio_fixture_replay_tests;
pub mod vad;

#[cfg(test)]
mod audio_contract_tests;
#[cfg(test)]
mod meeting_output_public_contract_tests;

use serde::{Deserialize, Serialize};

pub const TARGET_SAMPLE_RATE_HZ: u32 = 16_000;
pub const TARGET_CHANNELS: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioFrame {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

// Single shared sample-count -> milliseconds conversion for the audio engine.
// Truncation to u32 ms is intentional and identical to the former per-file copies.
pub(crate) fn duration_ms(sample_count: usize, sample_rate_hz: u32) -> u32 {
    if sample_rate_hz == 0 {
        return 0;
    }
    ((sample_count as u64 * 1_000) / sample_rate_hz as u64) as u32
}
