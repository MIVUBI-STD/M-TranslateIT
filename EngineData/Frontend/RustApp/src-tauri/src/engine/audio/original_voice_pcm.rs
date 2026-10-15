//! Optional direct-microphone PCM handoff for the existing Meeting output.
//!
//! This is a bounded, nonblocking capture/output primitive. No second microphone
//! stream, no driver and no AI pipeline are introduced. Until a native output
//! owner explicitly calls \`begin\`, the callback tap does nothing.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, TryLockError};

// Keep passthrough latency bounded even when the producer briefly outruns output.
const MAX_QUEUE_MS: usize = 250;
const MAX_MONO_FRAMES: usize = 48_000; // 250 ms at the supported 192 kHz upper bound
static ACTIVE: AtomicBool = AtomicBool::new(false);
static MUTED: AtomicBool = AtomicBool::new(false);
static GENERATION: AtomicU64 = AtomicU64::new(0);
static SOURCE_EPOCH: AtomicU64 = AtomicU64::new(0);
static BUFFER: OnceLock<Mutex<DirectPcm>> = OnceLock::new();

struct DirectPcm {
    samples: VecDeque<f32>,
    sample_rate_hz: u32,
    phase: f64,
    dropped_frames: u64,
}

impl DirectPcm {
    fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(MAX_MONO_FRAMES),
            sample_rate_hz: 0,
            phase: 0.0,
            dropped_frames: 0,
        }
    }

    fn clear(&mut self) {
        self.samples.clear();
        self.sample_rate_hz = 0;
        self.phase = 0.0;
    }

    fn push(&mut self, values: &[f32], sample_rate_hz: u32) {
        if !(8_000..=192_000).contains(&sample_rate_hz) || values.is_empty() {
            return;
        }
        if self.sample_rate_hz != sample_rate_hz {
            self.clear(); // Never splice samples from incompatible device formats.
            self.sample_rate_hz = sample_rate_hz;
        }
        let limit = ((sample_rate_hz as usize * MAX_QUEUE_MS) / 1000)
            .min(MAX_MONO_FRAMES).max(1);
        let values = if values.len() > limit {
            self.dropped_frames = self.dropped_frames.saturating_add((values.len() - limit) as u64);
            &values[values.len() - limit..]
        } else {
            values
        };
        let overflow = self.samples.len().saturating_add(values.len()).saturating_sub(limit);
        for _ in 0..overflow {
            self.samples.pop_front();
        }
        if overflow > 0 {
            self.dropped_frames = self.dropped_frames.saturating_add(overflow as u64);
            self.phase = 0.0;
        }
        for &sample in values {
            self.samples.push_back(if sample.is_finite() { sample.clamp(-1.0, 1.0) } else { 0.0 });
        }
    }

    fn render_native<T: Copy>(
        &mut self,
        dest: &mut [T],
        target_rate_hz: u32,
        channels: u16,
        silence: T,
        convert: impl Fn(f32) -> T,
    ) {
        dest.fill(silence);
        if target_rate_hz == 0 || channels == 0 || self.sample_rate_hz == 0 {
            return;
        }
        let step = f64::from(self.sample_rate_hz) / f64::from(target_rate_hz);
        let channel_count = usize::from(channels);
        for frame in dest.chunks_exact_mut(channel_count) {
            let (Some(&left), Some(&right)) = (self.samples.front(), self.samples.get(1)) else {
                self.clear();
                return;
            };
            let mono = left + (right - left) * self.phase as f32;
            frame.fill(convert(mono));
            self.phase += step;
            while self.phase >= 1.0 {
                self.samples.pop_front();
                self.phase -= 1.0;
            }
        }
    }

    #[cfg(test)]
    fn render(&mut self, dest: &mut [f32], target_rate_hz: u32, channels: u16) {
        self.render_native(dest, target_rate_hz, channels, 0.0_f32, |sample| sample);
    }
}

fn buffer() -> &'static Mutex<DirectPcm> {
    BUFFER.get_or_init(|| Mutex::new(DirectPcm::new()))
}

/// One source epoch per authoritative Meeting generation.
pub fn register_translated(generation: u64) -> Result<(), &'static str> {
    if generation == 0 { return Err("original_voice:invalid_generation"); }
    ACTIVE.store(false, Ordering::Release);
    let mut pcm = buffer().lock().map_err(|_| "original_voice:buffer_poisoned")?;
    pcm.clear();
    GENERATION.store(generation, Ordering::Release);
    SOURCE_EPOCH.store(1, Ordering::Release);
    MUTED.store(false, Ordering::Release);
    Ok(())
}
/// The mute gate is independent of the selected source. When muted, the
/// microphone and TTS output callbacks can only submit silence.
pub fn is_output_muted(generation: u64) -> bool {
    generation != 0
        && GENERATION.load(Ordering::Acquire) == generation
        && MUTED.load(Ordering::Acquire)
}

/// Called only by the authoritative, serialized Meeting command.
pub fn set_output_muted(generation: u64, muted: bool) -> Result<(), &'static str> {
    if generation == 0 || GENERATION.load(Ordering::Acquire) != generation
        || SOURCE_EPOCH.load(Ordering::Acquire) == 0
    {
        return Err("meeting_mute:source_generation_unavailable");
    }
    // Ensure a pending TTS WAV can never be replayed on Unmute. Raw Original
    // output keeps its own stream epoch so the same CPAL stream can resume.
    if muted && !MUTED.swap(true, Ordering::AcqRel)
        && !ACTIVE.load(Ordering::Acquire)
    {
        SOURCE_EPOCH.fetch_update(Ordering::AcqRel, Ordering::Acquire, |epoch| epoch.checked_add(1))
            .map_err(|_| "meeting_mute:source_epoch_exhausted")?;
    }
    if muted {
        if let Some(queue) = BUFFER.get() {
            let mut state = queue.lock().map_err(|_| "meeting_mute:pcm_lock_failed")?;
            state.clear();
        }
    } else {
        MUTED.store(false, Ordering::Release);
    }
    Ok(())
}

pub fn is_original_active(generation: u64) -> bool {
    ACTIVE.load(Ordering::Acquire) && GENERATION.load(Ordering::Acquire) == generation
}
pub fn translated_epoch(generation: u64) -> Option<u64> {
    if GENERATION.load(Ordering::Acquire) != generation
        || ACTIVE.load(Ordering::Acquire) || MUTED.load(Ordering::Acquire)
    { return None; }
    let epoch = SOURCE_EPOCH.load(Ordering::Acquire);
    (epoch > 0).then_some(epoch)
}
pub fn translated_epoch_matches(generation: u64, epoch: u64) -> bool {
    translated_epoch(generation) == Some(epoch)
}
pub fn next_original_epoch(generation: u64) -> Result<u64, &'static str> {
    if generation == 0 || GENERATION.load(Ordering::Acquire) != generation
        || ACTIVE.load(Ordering::Acquire) {
        return Err("original_voice:source_not_translated");
    }
    SOURCE_EPOCH.load(Ordering::Acquire)
        .checked_add(1).ok_or("original_voice:source_epoch_exhausted")
}

/// Called only by the native Meeting output owner, never by an audio callback.
/// Rebinding clears previous session PCM before opening the original-voice stream.
pub fn begin(generation: u64, source_epoch: u64) -> Result<(), &'static str> {
    if generation == 0 || source_epoch == 0 {
        return Err("original_voice:invalid_generation_or_epoch");
    }
    ACTIVE.store(false, Ordering::Release);
    let mut state = buffer().lock().map_err(|_| "original_voice:buffer_poisoned")?;
    state.clear();
    GENERATION.store(generation, Ordering::Release);
    SOURCE_EPOCH.store(source_epoch, Ordering::Release);
    ACTIVE.store(true, Ordering::Release);
    Ok(())
}

/// Called by the native Meeting output owner to invalidate old PCM on switch/Stop.
pub fn stop(generation: u64) {
    if GENERATION.load(Ordering::Acquire) != generation {
        return;
    }
    if ACTIVE.swap(false, Ordering::AcqRel) {
        let _ = SOURCE_EPOCH.fetch_update(Ordering::AcqRel, Ordering::Acquire, |epoch| epoch.checked_add(1));
    }
    if let Some(queue) = BUFFER.get() {
        if let Ok(mut state) = queue.lock() {
            state.clear();
        }
    }
}

/// The existing physical microphone callback calls this after mono conversion.
/// In the default translated mode the hot path is one atomic read and returns.
pub fn try_push_mono(samples: &[f32], sample_rate_hz: u32) {
    if !ACTIVE.load(Ordering::Acquire) || MUTED.load(Ordering::Acquire) {
        return;
    }
    let Some(queue) = BUFFER.get() else { return };
    if let Ok(mut state) = queue.try_lock() {
        if ACTIVE.load(Ordering::Acquire) && !MUTED.load(Ordering::Acquire) {
            state.push(samples, sample_rate_hz);
        }
    }
}

/// Output callback: bounded, nonblocking, and silence-only when not authorized.
/// Supports the native f32/i16/u16 device formats without an intermediate allocation.
pub fn try_render_native<T: Copy>(
    dest: &mut [T],
    sample_rate_hz: u32,
    channels: u16,
    generation: u64,
    source_epoch: u64,
    silence: T,
    convert: impl Fn(f32) -> T,
) {
    dest.fill(silence);
    if !ACTIVE.load(Ordering::Acquire) || MUTED.load(Ordering::Acquire)
        || GENERATION.load(Ordering::Acquire) != generation
        || SOURCE_EPOCH.load(Ordering::Acquire) != source_epoch
    {
        return;
    }
    let Some(queue) = BUFFER.get() else { return };
    match queue.try_lock() {
        Ok(mut state) => {
            if ACTIVE.load(Ordering::Acquire) && !MUTED.load(Ordering::Acquire)
                && GENERATION.load(Ordering::Acquire) == generation
                && SOURCE_EPOCH.load(Ordering::Acquire) == source_epoch
            {
                state.render_native(dest, sample_rate_hz, channels, silence, convert);
            }
        }
        Err(TryLockError::WouldBlock | TryLockError::Poisoned(_)) => {}
    }
    if !ACTIVE.load(Ordering::Acquire) || MUTED.load(Ordering::Acquire)
        || GENERATION.load(Ordering::Acquire) != generation
        || SOURCE_EPOCH.load(Ordering::Acquire) != source_epoch
    {
        dest.fill(silence);
    }
}

#[cfg(test)]
mod tests {
    use super::{DirectPcm, MAX_MONO_FRAMES, MAX_QUEUE_MS};
    #[test]
    fn silent_underrun_never_replays_old_samples() {
        let mut pcm = DirectPcm::new();
        pcm.push(&[0.4, 0.4, 0.4], 48_000);
        let mut out = [0.0; 8];
        pcm.render(&mut out, 48_000, 1);
        assert_eq!(out[0], 0.4);
        assert_eq!(&out[2..], &[0.0; 6]);
        pcm.render(&mut out, 48_000, 1);
        assert_eq!(out, [0.0; 8]);
    }
    #[test]
    fn mono_is_copied_to_stereo() {
        let mut pcm = DirectPcm::new();
        pcm.push(&[0.5, -0.5, 0.0], 48_000);
        let mut out = [0.0; 4];
        pcm.render(&mut out, 48_000, 2);
        assert_eq!(out, [0.5, 0.5, -0.5, -0.5]);
    }
    #[test]
    fn resampling_is_bounded_and_interpolated() {
        let mut pcm = DirectPcm::new();
        pcm.push(&[0.0, 1.0, 0.0, -1.0], 24_000);
        let mut out = [0.0; 6];
        pcm.render(&mut out, 48_000, 1);
        assert_eq!(&out[..4], &[0.0, 0.5, 1.0, 0.5]);
    }
    #[test]
    fn overflow_and_format_change_do_not_mix_old_pcm() {
        let mut pcm = DirectPcm::new();
        let limit_48k = 48_000 * MAX_QUEUE_MS / 1000;
        pcm.push(&vec![0.25; MAX_MONO_FRAMES], 48_000);
        pcm.push(&[0.8, 0.9], 48_000);
        assert_eq!(pcm.samples.len(), limit_48k);
        assert_eq!(pcm.dropped_frames, (MAX_MONO_FRAMES - limit_48k + 2) as u64);
        pcm.push(&[0.1, 0.2], 44_100);
        assert_eq!(pcm.samples.len(), 2);
        assert_eq!(pcm.sample_rate_hz, 44_100);
    }
    #[test]
    fn backlog_is_bounded_by_input_rate_not_one_fixed_sample_count() {
        let mut pcm = DirectPcm::new();
        pcm.push(&vec![0.5; 48_000], 192_000);
        assert_eq!(pcm.samples.len(), 48_000);
        pcm.push(&vec![0.25; 48_000], 8_000);
        assert_eq!(pcm.samples.len(), 8_000 * MAX_QUEUE_MS / 1000);
        assert!(pcm.dropped_frames > 0);
    }
    #[test]
    fn source_epoch_blocks_stale_tts_across_mode_round_trip() {
        const GENERATION: u64 = 880_012;
        super::register_translated(GENERATION).expect("translation epoch initialized");
        let before = super::translated_epoch(GENERATION).expect("translated");
        assert!(super::translated_epoch_matches(GENERATION, before));
        let original_epoch = super::next_original_epoch(GENERATION).expect("next epoch");
        super::begin(GENERATION, original_epoch).expect("switch to original");
        assert!(super::is_original_active(GENERATION));
        assert!(!super::translated_epoch_matches(GENERATION, before));
        super::set_output_muted(GENERATION, true).expect("raw mute");
        assert!(super::is_output_muted(GENERATION));
        let mut silent = [1.0_f32; 8];
        super::try_render_native(&mut silent, 48_000, 1, GENERATION, original_epoch, 0.0, |x| x);
        assert_eq!(silent, [0.0; 8]);
        super::set_output_muted(GENERATION, false).expect("raw unmute");
        assert!(!super::is_output_muted(GENERATION));
        super::stop(GENERATION);
        let resumed = super::translated_epoch(GENERATION).expect("translated again");
        assert!(resumed > original_epoch);
        assert!(!super::translated_epoch_matches(GENERATION, before));
        super::set_output_muted(GENERATION, true).expect("translated mute");
        assert!(super::translated_epoch(GENERATION).is_none());
        super::set_output_muted(GENERATION, false).expect("translated unmute");
        assert!(super::translated_epoch(GENERATION).unwrap() > resumed);
    }
    #[test]
    fn malformed_samples_are_sanitized() {
        let mut pcm = DirectPcm::new();
        pcm.push(&[f32::NAN, 2.0, -2.0], 48_000);
        assert_eq!(pcm.samples.iter().copied().collect::<Vec<_>>(), vec![0.0, 1.0, -1.0]);
    }
}
