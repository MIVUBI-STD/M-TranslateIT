use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::evidence::AudioEvidenceReport;
use super::vad::{evaluate_vad_gate, runtime_vad_profile, RuntimeVadProfile};
use super::{duration_ms, AudioFrame, TARGET_CHANNELS, TARGET_SAMPLE_RATE_HZ};
use crate::engine::runtime_state::runtime_generation_is_authoritative;

// Internal safety bounds only. They are not product speech-boundary policy.
// An overlong in-progress utterance is dropped fail-closed rather than emitted as a
// partial segment merely to satisfy a limit. Pending finalized work remains bounded
// separately and prefers newer waiting speech over an older realtime backlog.
const MAX_IN_PROGRESS_UTTERANCE_MS: u32 = 60_000;
const MAX_PENDING_FINALIZED_UTTERANCES: usize = 2;
const LANE_YOU: &str = "you";
const LANE_INCOMING: &str = "incoming";

static OVERFLOW_DROPPED_UTTERANCES: AtomicU64 = AtomicU64::new(0);
static EVICTED_PENDING_UTTERANCES: AtomicU64 = AtomicU64::new(0);

// Silent-loss observability for bounded producer drops. These counters only make
// already-existing fail-closed discards visible through the outbound status payload;
// they do not change any drop/keep decision.
pub fn overflow_dropped_utterance_count() -> u64 {
    OVERFLOW_DROPPED_UTTERANCES.load(Ordering::Relaxed)
}

pub fn evicted_pending_utterance_count() -> u64 {
    EVICTED_PENDING_UTTERANCES.load(Ordering::Relaxed)
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn elapsed_millis(start: Instant, end: Instant) -> u64 {
    end.checked_duration_since(start)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone)]
pub struct FinalizedMeetingUtterance {
    pub session_id: String,
    pub sequence: u64,
    pub lane: String,
    pub generation: Option<u64>,
    pub utterance_id: u64,
    pub finalized_at: Instant,
    pub enqueued_at: Instant,
    pub finalized_unix_ms: u128,
    pub speech_boundary_ms: u64,
    pub speech_duration_ms: u64,
    pub finalization_ms: u64,
    pub frame: AudioFrame,
}

#[derive(Debug)]
struct FinalizedProducerState {
    session_id: String,
    generation: Option<u64>,
    lane: &'static str,
    sample_rate_hz: u32,
    profile: RuntimeVadProfile,
    pre_roll: VecDeque<f32>,
    in_utterance: bool,
    current_samples: Vec<f32>,
    speech_samples: usize,
    trailing_silence_samples: usize,
    overflowed: bool,
    next_utterance_id: u64,
    pending: VecDeque<FinalizedMeetingUtterance>,
}

struct FinalizedProducerSync {
    state: Mutex<Option<FinalizedProducerState>>,
    ready: Condvar,
}

#[derive(Debug)]
struct MeetingSequenceState {
    session_id: String,
    next_sequence: u64,
}

static FINALIZED_OUTBOUND_PRODUCER: OnceLock<FinalizedProducerSync> = OnceLock::new();
static FINALIZED_INCOMING_PRODUCER: OnceLock<FinalizedProducerSync> = OnceLock::new();
static FINALIZED_MEETING_SEQUENCE: OnceLock<Mutex<Option<MeetingSequenceState>>> = OnceLock::new();

fn outbound_sync() -> &'static FinalizedProducerSync {
    FINALIZED_OUTBOUND_PRODUCER.get_or_init(|| FinalizedProducerSync {
        state: Mutex::new(None),
        ready: Condvar::new(),
    })
}

fn incoming_sync() -> &'static FinalizedProducerSync {
    FINALIZED_INCOMING_PRODUCER.get_or_init(|| FinalizedProducerSync {
        state: Mutex::new(None),
        ready: Condvar::new(),
    })
}

fn sequence_store() -> &'static Mutex<Option<MeetingSequenceState>> {
    FINALIZED_MEETING_SEQUENCE.get_or_init(|| Mutex::new(None))
}

pub fn reset_finalized_meeting_sequence(session_id: &str) {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        clear_finalized_meeting_sequence();
        return;
    }
    if let Ok(mut guard) = sequence_store().lock() {
        *guard = Some(MeetingSequenceState {
            session_id: session_id.to_string(),
            next_sequence: 1,
        });
    }
}

pub fn clear_finalized_meeting_sequence() {
    if let Ok(mut guard) = sequence_store().lock() {
        *guard = None;
    }
}

fn allocate_meeting_sequence(session_id: &str) -> Option<u64> {
    let mut guard = sequence_store().lock().ok()?;
    let state = guard.as_mut()?;
    if state.session_id != session_id {
        return None;
    }
    let sequence = state.next_sequence;
    state.next_sequence = state.next_sequence.saturating_add(1);
    Some(sequence)
}

pub fn reset_finalized_outbound_utterance_producer(
    session_id: &str,
    generation: u64,
    sample_rate_hz: u32,
) {
    reset_producer(
        outbound_sync(),
        session_id,
        Some(generation),
        LANE_YOU,
        sample_rate_hz,
    );
}

pub fn clear_finalized_outbound_utterance_producer() {
    clear_producer(outbound_sync());
}

pub fn reset_finalized_incoming_utterance_producer(session_id: &str, sample_rate_hz: u32) {
    reset_producer(
        incoming_sync(),
        session_id,
        None,
        LANE_INCOMING,
        sample_rate_hz,
    );
}

pub fn clear_finalized_incoming_utterance_producer() {
    clear_producer(incoming_sync());
}

pub fn reset_finalized_incoming_speech_boundary() {
    let sync = incoming_sync();
    if let Ok(mut guard) = sync.state.lock() {
        if let Some(state) = guard.as_mut() {
            reset_current_utterance(state);
        }
    }
}

fn reset_producer(
    sync: &FinalizedProducerSync,
    session_id: &str,
    generation: Option<u64>,
    lane: &'static str,
    sample_rate_hz: u32,
) {
    let session_id = session_id.trim();
    if sample_rate_hz == 0 || session_id.is_empty() {
        clear_producer(sync);
        return;
    }

    let profile = runtime_vad_profile();
    let pre_roll_capacity = samples_for_duration(sample_rate_hz, profile.pre_roll_audio_ms);
    let initial_utterance_ms = profile
        .target_chunk_min_ms
        .max(profile.minimum_speech_duration_ms)
        .saturating_add(profile.pre_roll_audio_ms);
    let initial_utterance_capacity = samples_for_duration(sample_rate_hz, initial_utterance_ms);

    if let Ok(mut guard) = sync.state.lock() {
        *guard = Some(FinalizedProducerState {
            session_id: session_id.to_string(),
            generation,
            lane,
            sample_rate_hz,
            profile,
            // Reserve normal working capacity before the audio callback starts so
            // ordinary speech does not repeatedly grow these buffers in realtime.
            pre_roll: VecDeque::with_capacity(pre_roll_capacity),
            in_utterance: false,
            current_samples: Vec::with_capacity(initial_utterance_capacity),
            speech_samples: 0,
            trailing_silence_samples: 0,
            overflowed: false,
            next_utterance_id: 1,
            pending: VecDeque::new(),
        });
        sync.ready.notify_all();
    }
}

fn clear_producer(sync: &FinalizedProducerSync) {
    if let Ok(mut guard) = sync.state.lock() {
        *guard = None;
        sync.ready.notify_all();
    }
}

pub fn observe_finalized_outbound_f32_samples(
    samples: &[f32],
    sample_rate_hz: u32,
    source_channels: u16,
) {
    observe_f32(outbound_sync(), samples, sample_rate_hz, source_channels);
}

pub fn observe_finalized_outbound_mono_f32_samples(samples: &[f32], sample_rate_hz: u32) {
    observe_finalized_mono_samples(outbound_sync(), samples, sample_rate_hz);
}

pub fn observe_finalized_incoming_f32_samples(
    samples: &[f32],
    sample_rate_hz: u32,
    source_channels: u16,
) {
    observe_f32(incoming_sync(), samples, sample_rate_hz, source_channels);
}

pub fn observe_finalized_incoming_i16_samples(
    samples: &[i16],
    sample_rate_hz: u32,
    source_channels: u16,
) {
    observe_i16(incoming_sync(), samples, sample_rate_hz, source_channels);
}

pub fn observe_finalized_incoming_i32_samples(
    samples: &[i32],
    sample_rate_hz: u32,
    source_channels: u16,
) {
    let mono = convert_and_downmix(samples, source_channels, |sample| {
        (sample as f64 / i32::MAX as f64).clamp(-1.0, 1.0) as f32
    });
    observe_finalized_mono_samples(incoming_sync(), &mono, sample_rate_hz);
}

pub fn observe_finalized_incoming_i64_samples(
    samples: &[i64],
    sample_rate_hz: u32,
    source_channels: u16,
) {
    let mono = convert_and_downmix(samples, source_channels, |sample| {
        (sample as f64 / i64::MAX as f64).clamp(-1.0, 1.0) as f32
    });
    observe_finalized_mono_samples(incoming_sync(), &mono, sample_rate_hz);
}

pub fn observe_finalized_incoming_u8_samples(
    samples: &[u8],
    sample_rate_hz: u32,
    source_channels: u16,
) {
    let mono = convert_and_downmix(samples, source_channels, |sample| {
        ((sample as f32 / u8::MAX as f32) * 2.0 - 1.0).clamp(-1.0, 1.0)
    });
    observe_finalized_mono_samples(incoming_sync(), &mono, sample_rate_hz);
}

fn observe_f32(
    sync: &FinalizedProducerSync,
    samples: &[f32],
    sample_rate_hz: u32,
    source_channels: u16,
) {
    if source_channels <= 1 {
        observe_finalized_mono_samples(sync, samples, sample_rate_hz);
        return;
    }
    let mono = downmix_f32(samples, source_channels);
    observe_finalized_mono_samples(sync, &mono, sample_rate_hz);
}

fn observe_i16(
    sync: &FinalizedProducerSync,
    samples: &[i16],
    sample_rate_hz: u32,
    source_channels: u16,
) {
    let mono = convert_and_downmix(samples, source_channels, |sample| {
        (sample as f32 / i16::MAX as f32).clamp(-1.0, 1.0)
    });
    observe_finalized_mono_samples(sync, &mono, sample_rate_hz);
}

fn observe_finalized_mono_samples(
    sync: &FinalizedProducerSync,
    samples: &[f32],
    sample_rate_hz: u32,
) {
    if samples.is_empty() || sample_rate_hz == 0 {
        return;
    }

    let Ok(mut guard) = sync.state.lock() else {
        return;
    };

    let Some(state) = guard.as_ref() else {
        return;
    };
    if state
        .generation
        .map(|generation| !runtime_generation_is_authoritative(generation))
        .unwrap_or(false)
    {
        *guard = None;
        sync.ready.notify_all();
        return;
    }

    let Some(state) = guard.as_mut() else {
        return;
    };
    if state.sample_rate_hz != sample_rate_hz {
        state.sample_rate_hz = sample_rate_hz;
        state.pre_roll.clear();
        reset_current_utterance(state);
    }

    // AudioEvidenceReport sanitizes while reading, and ingest_observation sanitizes
    // only samples that must be retained. Avoid constructing a second safe_samples
    // Vec for every callback chunk.
    let evidence = AudioEvidenceReport::from_samples(samples);
    let gate = evaluate_vad_gate(evidence.clone(), &state.profile.gate);
    let speech_like = gate.accepted;

    let queued = ingest_observation(state, samples, &evidence, speech_like);
    if queued {
        sync.ready.notify_one();
    }
}

pub fn wait_take_finalized_outbound_utterance(
    generation: u64,
) -> Option<FinalizedMeetingUtterance> {
    let sync = outbound_sync();
    let mut guard = sync.state.lock().ok()?;

    loop {
        // A stale consumer must never clear producer state owned by a newer
        // generation. Confirm ownership first; only the matching producer may be
        // discarded after its own runtime authority has been revoked.
        if guard.as_ref().and_then(|state| state.generation) != Some(generation) {
            return None;
        }
        if !runtime_generation_is_authoritative(generation) {
            *guard = None;
            sync.ready.notify_all();
            return None;
        }

        let state = guard.as_mut()?;
        if let Some(utterance) = state.pending.pop_front() {
            return Some(utterance);
        }

        guard = sync.ready.wait(guard).ok()?;
    }
}

pub fn wait_take_finalized_incoming_utterance(
    session_id: &str,
) -> Option<FinalizedMeetingUtterance> {
    let sync = incoming_sync();
    let mut guard = sync.state.lock().ok()?;

    loop {
        let Some(state) = guard.as_mut() else {
            return None;
        };
        if state.session_id != session_id || state.lane != LANE_INCOMING {
            return None;
        }
        if let Some(utterance) = state.pending.pop_front() {
            return Some(utterance);
        }
        guard = sync.ready.wait(guard).ok()?;
    }
}

pub fn try_take_finalized_incoming_utterance(
    session_id: &str,
) -> Option<FinalizedMeetingUtterance> {
    let sync = incoming_sync();
    let mut guard = sync.state.lock().ok()?;
    let state = guard.as_mut()?;
    if state.session_id != session_id || state.lane != LANE_INCOMING {
        return None;
    }
    state.pending.pop_front()
}

fn ingest_observation(
    state: &mut FinalizedProducerState,
    samples: &[f32],
    evidence: &AudioEvidenceReport,
    speech_like: bool,
) -> bool {
    if !state.in_utterance {
        if speech_like {
            state.in_utterance = true;
            state
                .current_samples
                .reserve(state.pre_roll.len() + samples.len());
            state.current_samples.extend(state.pre_roll.drain(..));
            extend_safe_samples(&mut state.current_samples, samples);
            state.speech_samples = samples.len();
            state.trailing_silence_samples = 0;
            state.overflowed = false;
        } else {
            append_pre_roll(state, samples);
        }
        return false;
    }

    if !state.overflowed {
        extend_safe_samples(&mut state.current_samples, samples);
        if duration_ms(state.current_samples.len(), state.sample_rate_hz)
            > MAX_IN_PROGRESS_UTTERANCE_MS
        {
            // Never turn this safety bound into a forced speech boundary. Discard the
            // overlong in-progress payload and wait for a natural silence reset.
            state.current_samples.clear();
            state.overflowed = true;
        }
    }

    if speech_like {
        state.speech_samples = state.speech_samples.saturating_add(samples.len());
        state.trailing_silence_samples = 0;
        return false;
    }

    state.trailing_silence_samples = state.trailing_silence_samples.saturating_add(samples.len());
    let speech_duration_ms = duration_ms(state.speech_samples, state.sample_rate_hz);
    let silence_duration_ms = duration_ms(state.trailing_silence_samples, state.sample_rate_hz);
    let required_silence_ms = adaptive_end_silence_ms(&state.profile, evidence);

    if silence_duration_ms < required_silence_ms {
        return false;
    }

    if speech_duration_ms < state.profile.minimum_speech_duration_ms || state.overflowed {
        if state.overflowed {
            OVERFLOW_DROPPED_UTTERANCES.fetch_add(1, Ordering::Relaxed);
        }
        reset_current_utterance(state);
        return false;
    }

    finalize_current_utterance(state)
}

fn finalize_current_utterance(state: &mut FinalizedProducerState) -> bool {
    if state
        .generation
        .map(|generation| !runtime_generation_is_authoritative(generation))
        .unwrap_or(false)
    {
        reset_current_utterance(state);
        return false;
    }

    let speech_end = state.current_samples.len().saturating_sub(
        state
            .trailing_silence_samples
            .min(state.current_samples.len()),
    );
    if speech_end == 0 {
        reset_current_utterance(state);
        return false;
    }
    let speech_duration_ms = duration_ms(speech_end, state.sample_rate_hz);

    // PR-052 begins at detected finalized-utterance end. Capture that point before
    // evidence validation/resampling. The preceding VAD/silence boundary is tracked
    // separately so target testing can distinguish segmentation delay from outbound
    // processing latency.
    let finalized_at = Instant::now();
    let finalized_unix_ms = current_unix_ms();
    let speech_boundary_ms = u64::from(duration_ms(
        state.trailing_silence_samples,
        state.sample_rate_hz,
    ));

    let speech_evidence = AudioEvidenceReport::from_samples(&state.current_samples[..speech_end]);
    let speech_gate = evaluate_vad_gate(speech_evidence, &state.profile.gate);
    if !speech_gate.accepted {
        reset_current_utterance(state);
        return false;
    }

    let source_samples = std::mem::take(&mut state.current_samples);
    let target_samples = if state.sample_rate_hz == TARGET_SAMPLE_RATE_HZ {
        source_samples
    } else {
        resample_linear(&source_samples, state.sample_rate_hz, TARGET_SAMPLE_RATE_HZ)
    };
    if target_samples.is_empty() {
        reset_current_utterance(state);
        return false;
    }

    let Some(sequence) = allocate_meeting_sequence(&state.session_id) else {
        reset_current_utterance(state);
        return false;
    };
    let utterance_id = state.next_utterance_id;
    state.next_utterance_id = state.next_utterance_id.saturating_add(1);
    // The consumer serializes retained work FIFO. If it falls behind, evict only
    // the oldest still-waiting finalized utterance so the bounded queue does not
    // preserve an increasingly stale realtime backlog at the expense of current
    // speech. Already-running output is not preempted here.
    while state.pending.len() >= MAX_PENDING_FINALIZED_UTTERANCES {
        let _ = state.pending.pop_front();
        EVICTED_PENDING_UTTERANCES.fetch_add(1, Ordering::Relaxed);
    }

    let enqueued_at = Instant::now();
    let finalization_ms = elapsed_millis(finalized_at, enqueued_at);
    state.pending.push_back(FinalizedMeetingUtterance {
        session_id: state.session_id.clone(),
        sequence,
        lane: state.lane.to_string(),
        generation: state.generation,
        utterance_id,
        finalized_at,
        enqueued_at,
        finalized_unix_ms,
        speech_boundary_ms,
        speech_duration_ms: u64::from(speech_duration_ms),
        finalization_ms,
        frame: AudioFrame {
            sample_rate_hz: TARGET_SAMPLE_RATE_HZ,
            channels: TARGET_CHANNELS,
            samples: target_samples,
        },
    });

    reset_current_utterance(state);
    true
}

fn adaptive_end_silence_ms(profile: &RuntimeVadProfile, evidence: &AudioEvidenceReport) -> u32 {
    let base = profile.minimum_silence_duration_ms.max(1);
    let rms_ratio = if profile.gate.min_rms > 0.0 {
        evidence.rms / profile.gate.min_rms
    } else {
        0.0
    };
    let peak_ratio = if profile.gate.min_peak > 0.0 {
        evidence.peak / profile.gate.min_peak
    } else {
        0.0
    };
    let boundary_ambiguity = rms_ratio.max(peak_ratio).clamp(0.0, 1.0);
    let adaptive_extra = (base as f32 * boundary_ambiguity).round() as u32;
    base.saturating_add(adaptive_extra)
        .min(profile.target_chunk_min_ms.max(base))
}

fn append_pre_roll(state: &mut FinalizedProducerState, samples: &[f32]) {
    state
        .pre_roll
        .extend(samples.iter().map(|sample| safe_sample(*sample)));
    let max_samples = samples_for_duration(state.sample_rate_hz, state.profile.pre_roll_audio_ms);
    while state.pre_roll.len() > max_samples {
        let _ = state.pre_roll.pop_front();
    }
}

fn extend_safe_samples(target: &mut Vec<f32>, samples: &[f32]) {
    target.extend(samples.iter().map(|sample| safe_sample(*sample)));
}

fn reset_current_utterance(state: &mut FinalizedProducerState) {
    state.in_utterance = false;
    state.current_samples.clear();
    state.speech_samples = 0;
    state.trailing_silence_samples = 0;
    state.overflowed = false;
    state.pre_roll.clear();
}

fn convert_and_downmix<T: Copy>(
    samples: &[T],
    source_channels: u16,
    convert: impl Fn(T) -> f32,
) -> Vec<f32> {
    let channel_count = usize::from(source_channels.max(1));
    if channel_count == 1 {
        return samples.iter().copied().map(convert).map(safe_sample).collect();
    }

    samples
        .chunks(channel_count)
        .map(|frame| {
            let sum = frame
                .iter()
                .copied()
                .map(&convert)
                .map(safe_sample)
                .sum::<f32>();
            sum / frame.len().max(1) as f32
        })
        .collect()
}

fn downmix_f32(samples: &[f32], source_channels: u16) -> Vec<f32> {
    let channel_count = usize::from(source_channels.max(1));
    if channel_count == 1 {
        return samples.iter().map(|sample| safe_sample(*sample)).collect();
    }

    samples
        .chunks(channel_count)
        .map(|frame| {
            let sum = frame.iter().map(|sample| safe_sample(*sample)).sum::<f32>();
            sum / frame.len().max(1) as f32
        })
        .collect()
}

fn resample_linear(samples: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if samples.is_empty() || source_rate == 0 || target_rate == 0 {
        return Vec::new();
    }
    if source_rate == target_rate {
        return samples.to_vec();
    }

    let target_len =
        ((samples.len() as u64 * target_rate as u64) / source_rate as u64).max(1) as usize;
    if samples.len() == 1 {
        return vec![samples[0]; target_len];
    }

    let ratio = source_rate as f64 / target_rate as f64;
    (0..target_len)
        .map(|index| {
            let source_position = index as f64 * ratio;
            let lower = source_position.floor() as usize;
            let upper = (lower + 1).min(samples.len() - 1);
            let fraction = (source_position - lower as f64) as f32;
            let lower_sample = samples[lower];
            let upper_sample = samples[upper];
            (lower_sample + (upper_sample - lower_sample) * fraction).clamp(-1.0, 1.0)
        })
        .collect()
}

fn samples_for_duration(sample_rate_hz: u32, duration_ms: u32) -> usize {
    ((sample_rate_hz.max(1) as u64 * duration_ms as u64) / 1_000) as usize
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

    // Both scenarios run inside one test fn because the producer state lives in
    // process-wide statics; parallel unit tests would race on those mutexes.

    #[test]
    fn incoming_lane_segmentation_eviction_and_overflow_are_observable() {
        // Scenario 1: three finalized utterances against the bounded pending
        // queue. The oldest must be evicted (observable counter) instead of
        // growing an unbounded realtime backlog.
        clear_finalized_meeting_sequence();
        clear_finalized_incoming_utterance_producer();
        reset_finalized_meeting_sequence("sess-evict");
        reset_finalized_incoming_utterance_producer("sess-evict", 16_000);

        let evicted_before = evicted_pending_utterance_count();
        let overflow_before = overflow_dropped_utterance_count();

        let mut speech = Vec::new();
        for i in 0..(16_000 * 300 / 1000) {
            speech.push(0.3 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 16_000.0).sin());
        }
        let silence = vec![0.0f32; 16_000 * 400 / 1000];

        for _ in 0..3 {
            observe_finalized_incoming_f32_samples(&speech, 16_000, 1);
            observe_finalized_incoming_f32_samples(&silence, 16_000, 1);
        }

        assert_eq!(evicted_pending_utterance_count() - evicted_before, 1, "oldest finalized utterance must be evicted once the third one arrives");
        assert_eq!(overflow_dropped_utterance_count() - overflow_before, 0);

        clear_finalized_incoming_utterance_producer();
        clear_finalized_meeting_sequence();

        // Scenario 2: continuous speech longer than the 60 s in-progress safety
        // bound is discarded and reported through the overflow counter rather
        // than forced into a fake boundary.
        clear_finalized_meeting_sequence();
        clear_finalized_incoming_utterance_producer();
        reset_finalized_meeting_sequence("sess-overflow");
        reset_finalized_incoming_utterance_producer("sess-overflow", 8_000);

        let evicted_before = evicted_pending_utterance_count();
        let overflow_before = overflow_dropped_utterance_count();

        let chunk: Vec<f32> = (0..(8_000 * 600 / 1000))
            .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 8_000.0).sin())
            .collect();
        // 101 chunks x 600 ms = 60.6 s of unbroken speech > 60 s ceiling.
        for _ in 0..101 {
            observe_finalized_incoming_f32_samples(&chunk, 8_000, 1);
        }
        observe_finalized_incoming_f32_samples(&silence_of(8_000, 400), 8_000, 1);

        assert_eq!(overflow_dropped_utterance_count() - overflow_before, 1, "overlong speech must land in the overflow counter");
        assert_eq!(evicted_pending_utterance_count() - evicted_before, 0);

        clear_finalized_incoming_utterance_producer();
        clear_finalized_meeting_sequence();
    }

    #[test]
    fn preroll_bound_resample_and_safe_retention_are_deterministic() {
        let profile = runtime_vad_profile();
        let mut state = FinalizedProducerState {
            session_id: "pure-contract".to_string(),
            generation: None,
            lane: LANE_INCOMING,
            sample_rate_hz: 16_000,
            profile,
            pre_roll: VecDeque::new(),
            in_utterance: false,
            current_samples: Vec::new(),
            speech_samples: 0,
            trailing_silence_samples: 0,
            overflowed: false,
            next_utterance_id: 1,
            pending: VecDeque::new(),
        };
        let max_pre_roll = samples_for_duration(
            state.sample_rate_hz,
            state.profile.pre_roll_audio_ms,
        );
        append_pre_roll(&mut state, &vec![0.0; max_pre_roll.saturating_add(16_000)]);
        assert_eq!(state.pre_roll.len(), max_pre_roll);

        let resampled = resample_linear(&[0.0, 0.25, -0.25, 0.0], 8_000, 16_000);
        assert_eq!(resampled.len(), 8);
        assert!(resampled.iter().all(|sample| (-1.0..=1.0).contains(sample)));
        assert_eq!(safe_sample(f32::NAN), 0.0);
        assert_eq!(safe_sample(f32::INFINITY), 0.0);
        assert_eq!(safe_sample(2.0), 1.0);

        let mut retained = Vec::new();
        extend_safe_samples(&mut retained, &[f32::NAN, 2.0, -2.0, 0.25]);
        assert_eq!(retained, vec![0.0, 1.0, -1.0, 0.25]);

        let i16_mono = convert_and_downmix(&[i16::MAX, 0, i16::MIN], 1, |sample| {
            (sample as f32 / i16::MAX as f32).clamp(-1.0, 1.0)
        });
        assert_eq!(i16_mono, vec![1.0, 0.0, -1.0]);

        let stereo = convert_and_downmix(&[i16::MAX, 0, 0, i16::MIN], 2, |sample| {
            (sample as f32 / i16::MAX as f32).clamp(-1.0, 1.0)
        });
        assert_eq!(stereo.len(), 2);
        assert!((stereo[0] - 0.5).abs() < 0.0001);
        assert!((stereo[1] + 0.5).abs() < 0.0001);
    }

    fn silence_of(rate: u32, ms: u32) -> Vec<f32> {
        vec![0.0f32; rate as usize * ms as usize / 1000]
    }
}
