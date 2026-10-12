use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use super::evidence::AudioEvidenceReport;
use super::vad::{evaluate_vad_gate, runtime_vad_profile, VadGateResult};
use super::{duration_ms, TARGET_CHANNELS, TARGET_SAMPLE_RATE_HZ};

const MAX_BUFFER_MS: u32 = 2_000;
const MIN_VAD_MS: u32 = 160;
const MIN_SEGMENT_MS: u32 = 320;

#[derive(Debug, Clone, Serialize)]
pub struct LiveAudioBufferStatusReport {
    pub has_audio: bool,
    pub sample_rate_hz: Option<u32>,
    pub source_channels: Option<u16>,
    pub buffer_channels: u16,
    pub target_sample_rate_hz: u32,
    pub target_channels: u16,
    pub buffered_samples: usize,
    pub max_buffer_samples: usize,
    pub buffered_duration_ms: u32,
    pub frames_received: u64,
    pub requires_resample_to_target: bool,
    pub source_downmixed_to_mono: bool,
    pub ready_for_vad: bool,
    pub ready_for_segment_pipeline: bool,
    pub ready_for_target_asr_frame: bool,
    pub evidence: AudioEvidenceReport,
    pub vad_result: VadGateResult,
    pub blocker: String,
    pub note: String,
}

#[derive(Debug, Clone)]
struct LiveAudioWindow {
    sample_rate_hz: u32,
    source_channels: u16,
    max_buffer_samples: usize,
    samples_mono: VecDeque<f32>,
    frames_received: u64,
}

static LIVE_AUDIO_WINDOW: OnceLock<Mutex<Option<LiveAudioWindow>>> = OnceLock::new();

pub fn reset_live_audio_buffer(
    sample_rate_hz: u32,
    source_channels: u16,
) -> LiveAudioBufferStatusReport {
    let max_buffer_samples = max_buffer_samples(sample_rate_hz);
    let window = LiveAudioWindow {
        sample_rate_hz,
        source_channels,
        max_buffer_samples,
        // Allocate the full two-second rolling window before the realtime callback
        // starts. The callback can then overwrite the bounded queue without growing
        // a Vec or shifting the retained tail on every trim.
        samples_mono: VecDeque::with_capacity(max_buffer_samples),
        frames_received: 0,
    };

    let store = LIVE_AUDIO_WINDOW.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = store.lock() {
        *guard = Some(window);
    }

    live_audio_buffer_status()
}

pub fn clear_live_audio_buffer() -> LiveAudioBufferStatusReport {
    let store = LIVE_AUDIO_WINDOW.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = store.lock() {
        *guard = None;
    }
    inactive_status(
        "live_audio_buffer:cleared",
        "Live audio rolling buffer was cleared.",
    )
}

pub fn append_live_f32_samples(samples: &[f32], sample_rate_hz: u32, source_channels: u16) {
    if source_channels <= 1 {
        append_live_mono_f32_samples(samples, sample_rate_hz, source_channels.max(1));
        return;
    }
    let mono = downmix_f32(samples, source_channels);
    append_live_mono_f32_samples(&mono, sample_rate_hz, source_channels);
}

pub fn append_live_mono_f32_samples(
    samples_mono: &[f32],
    sample_rate_hz: u32,
    source_channels: u16,
) {
    append_mono_samples(samples_mono, sample_rate_hz, source_channels.max(1));
}

pub fn live_audio_buffer_status() -> LiveAudioBufferStatusReport {
    let store = LIVE_AUDIO_WINDOW.get_or_init(|| Mutex::new(None));
    match store.lock() {
        Ok(guard) => build_status(guard.as_ref()),
        Err(_) => inactive_status(
            "live_audio_buffer:state_lock_failed",
            "Live audio buffer state lock failed while reading status.",
        ),
    }
}

fn append_mono_samples(samples_mono: &[f32], sample_rate_hz: u32, source_channels: u16) {
    if samples_mono.is_empty() {
        return;
    }

    let store = LIVE_AUDIO_WINDOW.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = store.lock() else {
        return;
    };

    if guard.is_none() {
        let max_buffer_samples = max_buffer_samples(sample_rate_hz);
        *guard = Some(LiveAudioWindow {
            sample_rate_hz,
            source_channels,
            max_buffer_samples,
            samples_mono: VecDeque::with_capacity(max_buffer_samples),
            frames_received: 0,
        });
    }

    let Some(window) = guard.as_mut() else {
        return;
    };

    if window.sample_rate_hz != sample_rate_hz || window.source_channels != source_channels {
        window.sample_rate_hz = sample_rate_hz;
        window.source_channels = source_channels;
        window.max_buffer_samples = max_buffer_samples(sample_rate_hz);
        window.samples_mono = VecDeque::with_capacity(window.max_buffer_samples);
        window.frames_received = 0;
    }

    window.frames_received = window
        .frames_received
        .saturating_add(samples_mono.len() as u64);
    window
        .samples_mono
        .extend(samples_mono.iter().map(|sample| safe_sample(*sample)));

    let excess = window
        .samples_mono
        .len()
        .saturating_sub(window.max_buffer_samples);
    if excess > 0 {
        // VecDeque front removal does not memmove the retained tail. The old Vec
        // drain(0..excess) path did exactly that work inside the audio callback.
        drop(window.samples_mono.drain(..excess));
    }
}

fn build_status(window: Option<&LiveAudioWindow>) -> LiveAudioBufferStatusReport {
    let Some(window) = window else {
        return inactive_status(
            "live_audio_buffer:not_active",
            "Live audio rolling buffer is not active.",
        );
    };

    let buffered_samples = window.samples_mono.len();
    let buffered_duration_ms = duration_ms(buffered_samples, window.sample_rate_hz);
    // Diagnostics/status are allowed to materialize a chronological snapshot. This
    // keeps that copy out of the realtime callback where latency variance matters.
    let snapshot = window.samples_mono.iter().copied().collect::<Vec<_>>();
    let evidence = AudioEvidenceReport::from_samples(&snapshot);
    // Diagnostics must evaluate with the SAME active gate profile the production
    // speech pipeline uses, not a second divergent default threshold set.
    let vad_result = evaluate_vad_gate(evidence.clone(), &runtime_vad_profile().gate);
    let requires_resample_to_target = window.sample_rate_hz != TARGET_SAMPLE_RATE_HZ;
    let source_downmixed_to_mono = window.source_channels != TARGET_CHANNELS;
    let ready_for_vad = buffered_duration_ms >= MIN_VAD_MS && vad_result.accepted;
    let ready_for_segment_pipeline = ready_for_vad && buffered_duration_ms >= MIN_SEGMENT_MS;
    let ready_for_target_asr_frame = ready_for_segment_pipeline;

    let blocker = if !ready_for_vad && buffered_duration_ms < MIN_VAD_MS {
        "live_audio_buffer:not_enough_audio_for_vad".to_string()
    } else if !ready_for_vad {
        format!("live_audio_buffer:vad_rejected:{}", vad_result.reason)
    } else if !ready_for_segment_pipeline {
        "live_audio_buffer:not_enough_audio_for_segment".to_string()
    } else {
        String::new()
    };

    let note = if ready_for_target_asr_frame {
        format!(
            "Live audio buffer is ready for target ASR frame extraction. duration_ms={}, source_rate={}, resample_required={}, source_downmixed_to_mono={}, rms={:.5}, peak={:.5}",
            buffered_duration_ms,
            window.sample_rate_hz,
            requires_resample_to_target,
            source_downmixed_to_mono,
            evidence.rms,
            evidence.peak
        )
    } else if ready_for_vad {
        format!(
            "Live audio has passed VAD, but segment duration is not ready yet. duration_ms={}, blocker={}",
            buffered_duration_ms, blocker
        )
    } else {
        format!(
            "Live audio buffer is collecting microphone audio. duration_ms={}, blocker={}",
            buffered_duration_ms, blocker
        )
    };

    LiveAudioBufferStatusReport {
        has_audio: buffered_samples > 0,
        sample_rate_hz: Some(window.sample_rate_hz),
        source_channels: Some(window.source_channels),
        buffer_channels: TARGET_CHANNELS,
        target_sample_rate_hz: TARGET_SAMPLE_RATE_HZ,
        target_channels: TARGET_CHANNELS,
        buffered_samples,
        max_buffer_samples: window.max_buffer_samples,
        buffered_duration_ms,
        frames_received: window.frames_received,
        requires_resample_to_target,
        source_downmixed_to_mono,
        ready_for_vad,
        ready_for_segment_pipeline,
        ready_for_target_asr_frame,
        evidence,
        vad_result,
        blocker,
        note,
    }
}

fn downmix_f32(samples: &[f32], source_channels: u16) -> Vec<f32> {
    let channel_count = usize::from(source_channels.max(1));
    samples
        .chunks(channel_count)
        .map(|frame| {
            let sum = frame.iter().map(|sample| safe_sample(*sample)).sum::<f32>();
            sum / frame.len().max(1) as f32
        })
        .collect()
}

fn max_buffer_samples(sample_rate_hz: u32) -> usize {
    let safe_rate = sample_rate_hz.max(1);
    ((safe_rate as u64 * MAX_BUFFER_MS as u64) / 1_000) as usize
}

fn safe_sample(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

fn inactive_status(blocker: &str, note: &str) -> LiveAudioBufferStatusReport {
    let evidence = AudioEvidenceReport::empty();
    let vad_result = VadGateResult {
        accepted: false,
        reason: blocker.to_string(),
        evidence: evidence.clone(),
    };

    LiveAudioBufferStatusReport {
        has_audio: false,
        sample_rate_hz: None,
        source_channels: None,
        buffer_channels: TARGET_CHANNELS,
        target_sample_rate_hz: TARGET_SAMPLE_RATE_HZ,
        target_channels: TARGET_CHANNELS,
        buffered_samples: 0,
        max_buffer_samples: 0,
        buffered_duration_ms: 0,
        frames_received: 0,
        requires_resample_to_target: false,
        source_downmixed_to_mono: false,
        ready_for_vad: false,
        ready_for_segment_pipeline: false,
        ready_for_target_asr_frame: false,
        evidence,
        vad_result,
        blocker: blocker.to_string(),
        note: note.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rolling_buffer_retains_only_newest_samples_in_chronological_order() {
        let _serial = crate::engine::runtime_state::runtime_test_guard();
        clear_live_audio_buffer();
        reset_live_audio_buffer(10, 1);
        let source = (0..30).map(|value| value as f32 / 30.0).collect::<Vec<_>>();
        append_live_mono_f32_samples(&source, 10, 1);

        let store = LIVE_AUDIO_WINDOW.get_or_init(|| Mutex::new(None));
        let guard = store.lock().expect("live buffer test lock");
        let window = guard.as_ref().expect("live buffer active");
        let retained = window.samples_mono.iter().copied().collect::<Vec<_>>();
        assert_eq!(retained.len(), 20);
        assert_eq!(retained, source[10..].to_vec());
        drop(guard);
        clear_live_audio_buffer();
    }
}
