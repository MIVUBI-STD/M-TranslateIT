use rubato::{FftFixedInOut, Resampler};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use super::evidence::AudioEvidenceReport;

pub const GUIDED_TAKE_SAMPLE_RATE_HZ: u32 = 32_000;
pub const GUIDED_TAKE_CHANNELS: u16 = 1;
const MAX_GUIDED_CAPTURE_MS: u64 = 60_000;
const RESAMPLE_CHUNK_FRAMES: usize = 1024;
const ACTIVE_SAMPLE_THRESHOLD: f32 = 128.0 / 32_768.0;
const MIN_ACTIVE_RMS: f32 = 256.0 / 32_768.0;
const MAX_DC_OFFSET: f32 = 2_048.0 / 32_768.0;
const MAX_CLIPPING_RATIO: f32 = 0.05;

#[derive(Debug, Clone, Serialize)]
pub struct GuidedTakeReview {
    pub line_id: u32,
    pub duration_ms: u64,
    pub quality_blocker: String,
}

pub struct CapturedGuidedTake {
    pub line_id: u32,
    pub samples_mono: Vec<f32>,
}

struct ActiveGuidedTake {
    line_id: u32,
    sample_rate_hz: Option<u32>,
    channels: Option<u16>,
    samples_mono: Vec<f32>,
    source_format_changed: bool,
    safety_limit_reached: bool,
}

static ACTIVE_GUIDED_TAKE: OnceLock<Mutex<Option<ActiveGuidedTake>>> = OnceLock::new();
static GUIDED_TAKE_ACTIVE: AtomicBool = AtomicBool::new(false);

fn store() -> &'static Mutex<Option<ActiveGuidedTake>> {
    ACTIVE_GUIDED_TAKE.get_or_init(|| Mutex::new(None))
}

pub fn active_guided_take_line_id() -> Option<u32> {
    if !GUIDED_TAKE_ACTIVE.load(Ordering::Acquire) {
        return None;
    }
    store()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|take| take.line_id))
}

pub fn arm_guided_take(line_id: u32) -> Result<(), String> {
    if line_id == 0 {
        return Err("voice_lab:invalid_guided_line".to_string());
    }
    let mut guard = store()
        .lock()
        .map_err(|_| "voice_lab:guided_capture_state_unavailable".to_string())?;
    if guard.is_some() {
        return Err("voice_lab:guided_capture_already_active".to_string());
    }
    *guard = Some(ActiveGuidedTake {
        line_id,
        sample_rate_hz: None,
        channels: None,
        samples_mono: Vec::new(),
        source_format_changed: false,
        safety_limit_reached: false,
    });
    GUIDED_TAKE_ACTIVE.store(true, Ordering::Release);
    Ok(())
}

pub fn cancel_guided_take() {
    // Meeting capture calls append_guided_f32 from its audio callback even though
    // My Voice recording is mutually exclusive with Meeting. Publish the cheap
    // inactive truth first so normal Meeting callbacks avoid this mutex entirely.
    GUIDED_TAKE_ACTIVE.store(false, Ordering::Release);
    if let Ok(mut guard) = store().lock() {
        *guard = None;
    }
}

pub fn append_guided_f32(data: &[f32], rate: u32, channels: u16) {
    if !GUIDED_TAKE_ACTIVE.load(Ordering::Acquire) {
        return;
    }
    append_mono(
        rate,
        channels,
        data.chunks_exact(usize::from(channels.max(1))).map(|frame| {
            frame.iter().copied().map(safe_sample).sum::<f32>() / frame.len().max(1) as f32
        }),
    );
}

fn append_mono(rate: u32, channels: u16, samples: impl Iterator<Item = f32>) {
    if rate == 0 || channels == 0 {
        return;
    }
    let Ok(mut guard) = store().lock() else {
        return;
    };
    let Some(take) = guard.as_mut() else {
        GUIDED_TAKE_ACTIVE.store(false, Ordering::Release);
        return;
    };
    match (take.sample_rate_hz, take.channels) {
        (None, None) => {
            take.sample_rate_hz = Some(rate);
            take.channels = Some(channels);
        }
        (Some(old_rate), Some(old_channels)) if old_rate != rate || old_channels != channels => {
            take.source_format_changed = true;
            return;
        }
        _ => {}
    }
    if take.safety_limit_reached {
        return;
    }
    let max_samples = (u64::from(rate) * MAX_GUIDED_CAPTURE_MS / 1_000) as usize;
    for sample in samples {
        if take.samples_mono.len() >= max_samples {
            take.safety_limit_reached = true;
            break;
        }
        take.samples_mono.push(safe_sample(sample));
    }
}

pub fn take_guided_audio() -> Result<(CapturedGuidedTake, GuidedTakeReview), String> {
    if !GUIDED_TAKE_ACTIVE.load(Ordering::Acquire) {
        return Err("voice_lab:no_active_guided_capture".to_string());
    }
    let take = store()
        .lock()
        .map_err(|_| "voice_lab:guided_capture_state_unavailable".to_string())?
        .take()
        .ok_or_else(|| "voice_lab:no_active_guided_capture".to_string())?;
    GUIDED_TAKE_ACTIVE.store(false, Ordering::Release);
    if take.source_format_changed {
        return Err("voice_lab:microphone_format_changed_during_take".to_string());
    }
    if take.safety_limit_reached {
        return Err("voice_lab:recording_safety_limit_reached".to_string());
    }
    let source_rate = take
        .sample_rate_hz
        .ok_or_else(|| "voice_lab:take_has_no_audio".to_string())?;
    if take.samples_mono.is_empty() {
        return Err("voice_lab:take_has_no_audio".to_string());
    }

    let samples_mono = resample_mono_fft(
        &take.samples_mono,
        source_rate,
        GUIDED_TAKE_SAMPLE_RATE_HZ,
    )?;
    let evidence = AudioEvidenceReport::from_samples(&samples_mono);
    let quality_blocker = guided_take_quality_blocker(&samples_mono, &evidence);
    let review = GuidedTakeReview {
        line_id: take.line_id,
        duration_ms: samples_mono.len() as u64 * 1_000 / u64::from(GUIDED_TAKE_SAMPLE_RATE_HZ),
        quality_blocker,
    };
    Ok((
        CapturedGuidedTake {
            line_id: take.line_id,
            samples_mono,
        },
        review,
    ))
}

fn guided_take_quality_blocker(samples: &[f32], evidence: &AudioEvidenceReport) -> String {
    if evidence.reason == "rejected_silence" {
        return "voice_lab:take_signal_unusable:rejected_silence".to_string();
    }
    if evidence.clipping_ratio >= MAX_CLIPPING_RATIO {
        return "voice_lab:take_signal_unusable:severe_clipping".to_string();
    }
    if samples.is_empty() {
        return "voice_lab:take_signal_unusable:rejected_silence".to_string();
    }

    let mut active_count = 0usize;
    let mut active_sum_square = 0.0_f64;
    let mut sum = 0.0_f64;
    for sample in samples {
        let value = f64::from(safe_sample(*sample));
        sum += value;
        if value.abs() > f64::from(ACTIVE_SAMPLE_THRESHOLD) {
            active_count += 1;
            active_sum_square += value * value;
        }
    }
    let active_rms = if active_count == 0 {
        0.0
    } else {
        (active_sum_square / active_count as f64).sqrt() as f32
    };
    if active_rms < MIN_ACTIVE_RMS {
        return "voice_lab:take_signal_unusable:signal_too_low".to_string();
    }
    let dc_offset = (sum / samples.len() as f64).abs() as f32;
    if dc_offset >= MAX_DC_OFFSET {
        return "voice_lab:take_signal_unusable:dc_offset_too_high".to_string();
    }
    String::new()
}


fn resample_mono_fft(samples: &[f32], source_rate: u32, target_rate: u32) -> Result<Vec<f32>, String> {
    if samples.is_empty() || source_rate == 0 || target_rate == 0 {
        return Err("voice_lab:take_has_no_audio".to_string());
    }
    if source_rate == target_rate {
        return Ok(samples.iter().copied().map(safe_sample).collect());
    }

    let mut resampler = FftFixedInOut::<f32>::new(
        source_rate as usize,
        target_rate as usize,
        RESAMPLE_CHUNK_FRAMES,
        1,
    )
    .map_err(|error| format!("voice_lab:resampler_create_failed:{error}"))?;
    let delay = resampler.output_delay();
    let expected_frames = (samples.len() as u128 * u128::from(target_rate)
        / u128::from(source_rate)) as usize;
    let mut output = Vec::with_capacity(
        expected_frames
            .saturating_add(delay)
            .saturating_add(resampler.output_frames_max()),
    );
    let mut offset = 0usize;

    while samples.len().saturating_sub(offset) >= resampler.input_frames_next() {
        let input_frames = resampler.input_frames_next();
        let input = [&samples[offset..offset + input_frames]];
        let chunk = resampler
            .process(&input, None)
            .map_err(|error| format!("voice_lab:resample_failed:{error}"))?;
        output.extend(chunk.into_iter().next().unwrap_or_default());
        offset = offset.saturating_add(input_frames);
    }

    if offset < samples.len() {
        let input = [&samples[offset..]];
        let chunk = resampler
            .process_partial(Some(&input), None)
            .map_err(|error| format!("voice_lab:resample_partial_failed:{error}"))?;
        output.extend(chunk.into_iter().next().unwrap_or_default());
    }

    let no_input: Option<&[&[f32]]> = None;
    let tail = resampler
        .process_partial(no_input, None)
        .map_err(|error| format!("voice_lab:resample_flush_failed:{error}"))?;
    output.extend(tail.into_iter().next().unwrap_or_default());

    let start = delay.min(output.len());
    let available = output.len().saturating_sub(start);
    if expected_frames == 0 || available < expected_frames {
        return Err("voice_lab:resample_output_incomplete".to_string());
    }
    output.drain(..start);
    output.truncate(expected_frames);
    output.iter_mut().for_each(|sample| *sample = safe_sample(*sample));
    Ok(output)
}

fn safe_sample(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    static TEST_SERIAL: Mutex<()> = Mutex::new(());

    #[test]
    fn stereo_48khz_take_is_downmixed_and_fft_resampled_to_32khz() {
        let _serial = TEST_SERIAL.lock().expect("guided take test lock");
        cancel_guided_take();
        assert!(active_guided_take_line_id().is_none());
        arm_guided_take(1).expect("arm guided take");
        assert_eq!(active_guided_take_line_id(), Some(1));
        let mut stereo = Vec::with_capacity(48_000 * 2);
        for index in 0..48_000 {
            let sample = (TAU * 440.0 * index as f32 / 48_000.0).sin() * 0.2;
            stereo.push(sample);
            stereo.push(sample * 0.9);
        }
        append_guided_f32(&stereo, 48_000, 2);
        let (captured, review) = take_guided_audio().expect("finalized guided take");
        assert!(active_guided_take_line_id().is_none());
        assert_eq!(captured.line_id, 1);
        assert_eq!(captured.samples_mono.len(), 32_000);
        assert!(review.duration_ms >= 999 && review.duration_ms <= 1_001);
        assert!(review.quality_blocker.is_empty());
    }

    #[test]
    fn clipped_take_is_reviewable_but_not_quality_acceptable() {
        let _serial = TEST_SERIAL.lock().expect("guided take test lock");
        cancel_guided_take();
        arm_guided_take(3).expect("arm guided take");
        let mut samples = vec![0.20_f32; 32_000];
        for sample in &mut samples[..2_000] {
            *sample = 1.0;
        }
        append_guided_f32(&samples, 32_000, 1);
        let (_, review) = take_guided_audio().expect("finalized clipped take");
        assert_eq!(
            review.quality_blocker,
            "voice_lab:take_signal_unusable:severe_clipping"
        );
    }

    #[test]
    fn very_low_take_is_reviewable_but_not_quality_acceptable() {
        let _serial = TEST_SERIAL.lock().expect("guided take test lock");
        cancel_guided_take();
        arm_guided_take(4).expect("arm guided take");
        let samples = (0..32_000)
            .map(|index| if index % 2 == 0 { 0.0065 } else { -0.0065 })
            .collect::<Vec<_>>();
        append_guided_f32(&samples, 32_000, 1);
        let (_, review) = take_guided_audio().expect("finalized low take");
        assert_eq!(
            review.quality_blocker,
            "voice_lab:take_signal_unusable:signal_too_low"
        );
    }

    #[test]
    fn large_dc_offset_take_is_reviewable_but_not_quality_acceptable() {
        let _serial = TEST_SERIAL.lock().expect("guided take test lock");
        cancel_guided_take();
        arm_guided_take(5).expect("arm guided take");
        let samples = (0..32_000)
            .map(|index| if index % 2 == 0 { 0.20 } else { 0.08 })
            .collect::<Vec<_>>();
        append_guided_f32(&samples, 32_000, 1);
        let (_, review) = take_guided_audio().expect("finalized dc-offset take");
        assert_eq!(
            review.quality_blocker,
            "voice_lab:take_signal_unusable:dc_offset_too_high"
        );
    }

    #[test]
    fn silent_take_is_reviewable_but_not_quality_acceptable() {
        let _serial = TEST_SERIAL.lock().expect("guided take test lock");
        cancel_guided_take();
        arm_guided_take(2).expect("arm guided take");
        append_guided_f32(&vec![0.0; 32_000], 32_000, 1);
        let (_, review) = take_guided_audio().expect("finalized silence");
        assert_eq!(
            review.quality_blocker,
            "voice_lab:take_signal_unusable:rejected_silence"
        );
    }
}
