use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::evidence::AudioEvidenceReport;
use super::vad::{evaluate_vad_gate, runtime_vad_profile, RuntimeVadProfile, VadGateResult};
use super::{duration_ms, AudioFrame, TARGET_CHANNELS, TARGET_SAMPLE_RATE_HZ};
use crate::engine::runtime_state::runtime_generation_is_authoritative;

// Internal safety bounds only. They are not product speech-boundary policy.
// Never force a speech boundary or truncate speech to satisfy a byte limit.
// Allow only a short, bounded *quiet* suffix to complete natural VAD finalization
// near the limit; that suffix is trimmed from the ASR WAV if necessary. Truly
// uninterrupted speech beyond the limit still fails closed and is counted.
// Pending finalized work is separately bounded and favors recent speech.
const MAX_IN_PROGRESS_UTTERANCE_MS: u32 = 60_000;
const MAX_FINALIZING_QUIET_OVERHANG_MS: u32 = 1_000;
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

pub fn reset_finalized_drop_counters() {
    OVERFLOW_DROPPED_UTTERANCES.store(0, Ordering::Relaxed);
    EVICTED_PENDING_UTTERANCES.store(0, Ordering::Relaxed);
}

/// (You observations, You max wait µs, incoming observations, incoming max wait µs).
/// Measures only callback-side mutex admission, NOT full audio QoS.
pub fn finalized_callback_lock_metrics() -> (u64, u64, u64, u64) {
    let you = outbound_sync();
    let incoming = incoming_sync();
    (
        you.callback_lock_observations.load(Ordering::Relaxed),
        you.callback_lock_wait_max_ns.load(Ordering::Relaxed) / 1_000,
        incoming.callback_lock_observations.load(Ordering::Relaxed),
        incoming.callback_lock_wait_max_ns.load(Ordering::Relaxed) / 1_000,
    )
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

// Clock of accepted mono samples at the canonical VAD producer, NOT
// device/OS timestamps. Frames dropped upstream cannot be recovered here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinalizedCaptureProvenance {
    pub audio_epoch: u64,
    pub input_rate_hz: u32,
    pub input_start_sample: u64,
    pub input_end_sample: u64, // exclusive; includes pre-roll and trailing silence
    pub asr_sample_count: u64, // actual 16-kHz frame sent to the WAV writer
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
    pub capture_provenance: Option<FinalizedCaptureProvenance>,
    pub frame: AudioFrame,
}

#[derive(Debug)]
struct FinalizedProducerState {
    session_id: String,
    generation: Option<u64>,
    lane: &'static str,
    sample_rate_hz: u32,
    capture_epoch: u64,
    input_sample_cursor: u64,
    current_input_start: Option<u64>,
    capture_error_signal: Option<Arc<AtomicU64>>,
    observed_error_epoch: u64,
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
    // Aggregate observation only, never microphone or meeting text.
    callback_lock_observations: AtomicU64,
    callback_lock_wait_max_ns: AtomicU64,
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
        callback_lock_observations: AtomicU64::new(0),
        callback_lock_wait_max_ns: AtomicU64::new(0),
    })
}

fn incoming_sync() -> &'static FinalizedProducerSync {
    FINALIZED_INCOMING_PRODUCER.get_or_init(|| FinalizedProducerSync {
        state: Mutex::new(None),
        ready: Condvar::new(),
        callback_lock_observations: AtomicU64::new(0),
        callback_lock_wait_max_ns: AtomicU64::new(0),
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

// Only stop the producer still owned by the requested Meeting generation.
// Returning false means cleanup could not be verified (e.g., poisoned mutex).
pub fn clear_finalized_outbound_utterance_producer_for_generation(generation: u64) -> bool {
    if generation == 0 {
        return false;
    }
    clear_producer_if(outbound_sync(), |producer| {
        producer.generation == Some(generation) && producer.lane == LANE_YOU
    })
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

// A delayed Stop from a previous Meeting cannot clear a newer incoming lane.
pub fn clear_finalized_incoming_utterance_producer_for_session(session_id: &str) -> bool {
    if session_id.is_empty() {
        return false;
    }
    clear_producer_if(incoming_sync(), |producer| {
        producer.session_id == session_id
            && producer.lane == LANE_INCOMING
            && producer.generation.is_none()
    })
}

// Reset the same producer on each outbound source change.
pub fn reset_finalized_outbound_speech_boundary(
    session_id: &str,
    generation: u64,
) -> Option<u64> {
    if !runtime_generation_is_authoritative(generation) {
        return None;
    }
    let sync = outbound_sync();
    let mut guard = sync.state.lock().ok()?;
    let state = guard.as_mut()?;
    if state.session_id != session_id
        || state.lane != LANE_YOU
        || state.generation != Some(generation)
    {
        return None;
    }
    state.capture_epoch = state.capture_epoch.saturating_add(1);
    state.input_sample_cursor = 0;
    reset_current_utterance(state);
    state.pre_roll.clear();
    state.pending.clear();
    sync.ready.notify_all();
    Some(state.capture_epoch)
}

// The canonical incoming VAD producer may outlive a replay consumer or a
// suppression guard from a previous Meeting. A delayed reset MUST NOT erase
// the speech boundary or accepted-sample clock of a new session.
pub fn reset_finalized_incoming_speech_boundary(session_id: &str) -> Option<u64> {
    let sync = incoming_sync();
    let mut guard = sync.state.lock().ok()?;
    let state = guard.as_mut()?;
    if state.session_id != session_id || state.lane != LANE_INCOMING {
        return None;
    }
    state.capture_epoch = state.capture_epoch.saturating_add(1);
    state.input_sample_cursor = 0;
    reset_current_utterance(state);
    Some(state.capture_epoch)
}

// Bind a per-stream CPAL error epoch BEFORE stream.play. Stale stream signals
// cannot reset the producer belonging to a newer session or generation.
fn bind_capture_error_signal(
    sync: &FinalizedProducerSync,
    session_id: &str,
    generation: Option<u64>,
    lane: &str,
    signal: Arc<AtomicU64>,
) -> bool {
    let Ok(mut guard) = sync.state.lock() else { return false };
    let Some(state) = guard.as_mut() else { return false };
    if state.session_id != session_id || state.generation != generation || state.lane != lane {
        return false;
    }
    state.observed_error_epoch = signal.load(Ordering::Acquire);
    state.capture_error_signal = Some(signal);
    true
}

pub fn bind_finalized_outbound_capture_errors(
    session_id: &str, generation: u64, signal: Arc<AtomicU64>,
) -> bool {
    runtime_generation_is_authoritative(generation)
        && bind_capture_error_signal(outbound_sync(), session_id, Some(generation), LANE_YOU, signal)
}

pub fn bind_finalized_incoming_capture_errors(
    session_id: &str, signal: Arc<AtomicU64>,
) -> bool {
    bind_capture_error_signal(incoming_sync(), session_id, None, LANE_INCOMING, signal)
}

// Invoked under the ALREADY-existing VAD producer mutex, never from the
// CPAL error callback. A detected error means unknown PCM loss, not a known
// number of missing samples.
fn apply_capture_discontinuity(state: &mut FinalizedProducerState) -> bool {
    let Some(signal) = state.capture_error_signal.as_ref() else { return false };
    let current = signal.load(Ordering::Acquire);
    if current == state.observed_error_epoch { return false; }
    state.observed_error_epoch = current;
    state.capture_epoch = state.capture_epoch.saturating_add(1);
    state.input_sample_cursor = 0;
    reset_current_utterance(state);
    true
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

    sync.callback_lock_observations.store(0, Ordering::Relaxed);
    sync.callback_lock_wait_max_ns.store(0, Ordering::Relaxed);
    if let Ok(mut guard) = sync.state.lock() {
        *guard = Some(FinalizedProducerState {
            session_id: session_id.to_string(),
            generation,
            lane,
            sample_rate_hz,
            capture_epoch: 1,
            input_sample_cursor: 0,
            current_input_start: None,
            capture_error_signal: None,
            observed_error_epoch: 0,
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

// The owner check and mutation must use the SAME producer lock. An identity
// precheck followed by unconditional clear would race a newer producer reset.
fn clear_producer_if(
    sync: &FinalizedProducerSync,
    belongs_to_request: impl FnOnce(&FinalizedProducerState) -> bool,
) -> bool {
    let Ok(mut guard) = sync.state.lock() else { return false };
    if guard.as_ref().is_some_and(belongs_to_request) {
        *guard = None;
        sync.ready.notify_all();
    }
    true
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

pub fn observe_finalized_incoming_mono_f32_samples(samples: &[f32], sample_rate_hz: u32) {
    observe_finalized_mono_samples(incoming_sync(), samples, sample_rate_hz);
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

    let lock_started = Instant::now();
    let locked = sync.state.lock();
    let wait_ns = lock_started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
    sync.callback_lock_observations.fetch_add(1, Ordering::Relaxed);
    sync.callback_lock_wait_max_ns.fetch_max(wait_ns, Ordering::Relaxed);
    let Ok(mut guard) = locked else {
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
    let _ = apply_capture_discontinuity(state);
    if state.sample_rate_hz != sample_rate_hz {
        state.sample_rate_hz = sample_rate_hz;
        state.capture_epoch = state.capture_epoch.saturating_add(1);
        state.input_sample_cursor = 0;
        state.pre_roll.clear();
        reset_current_utterance(state);
    }
    let Some(next_cursor) = state.input_sample_cursor.checked_add(samples.len() as u64) else {
        state.capture_epoch = state.capture_epoch.saturating_add(1);
        state.input_sample_cursor = 0;
        reset_current_utterance(state);
        return;
    };
    // Advance before finalization; this callback block is fully accepted by
    // the existing mono VAD owner, with no extra callback allocation.
    state.input_sample_cursor = next_cursor;

    // AudioEvidenceReport sanitizes while reading, and ingest_observation sanitizes
    // only samples that must be retained. Avoid constructing a second safe_samples
    // Vec for every callback chunk.
    let evidence = AudioEvidenceReport::from_samples(samples);
    let gate = evaluate_vad_gate(evidence.clone(), &state.profile.gate);
    let queued = ingest_observation(state, samples, &evidence, &gate);
    // Handle an error callback racing with the same VAD observation. Never
    // publish an uncertain just-finalized phrase as continuous speech.
    if apply_capture_discontinuity(state) {
        if queued { let _ = state.pending.pop_back(); }
        return;
    }
    if queued { sync.ready.notify_one(); }
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
        let state = guard.as_mut()?;
        if state.session_id != session_id || state.lane != LANE_INCOMING {
            return None;
        }
        if let Some(utterance) = state.pending.pop_front() {
            return Some(utterance);
        }
        guard = sync.ready.wait(guard).ok()?;
    }
}

// Incoming deferred ASR/translation may need another chance even when no
// further Meeting Sound utterance arrives. Reuse the existing producer condition
// variable; only the consumer with queued deferred work requests a bounded wait.
#[derive(Debug)]
pub enum IncomingTimedWait {
    Utterance(FinalizedMeetingUtterance),
    TimedOut,
    Stopped,
}

pub fn wait_take_finalized_incoming_utterance_for(
    session_id: &str,
    timeout: Duration,
) -> IncomingTimedWait {
    wait_take_finalized_incoming_utterance_for_sync(incoming_sync(), session_id, timeout)
}

fn wait_take_finalized_incoming_utterance_for_sync(
    sync: &FinalizedProducerSync,
    session_id: &str,
    timeout: Duration,
) -> IncomingTimedWait {
    let Ok(guard) = sync.state.lock() else {
        return IncomingTimedWait::Stopped;
    };
    let Ok((mut guard, _)) = sync.ready.wait_timeout_while(guard, timeout, |producer| {
        producer.as_ref().is_some_and(|state| {
            state.session_id == session_id
                && state.lane == LANE_INCOMING
                && state.pending.is_empty()
        })
    }) else {
        return IncomingTimedWait::Stopped;
    };

    let Some(state) = guard.as_mut() else {
        return IncomingTimedWait::Stopped;
    };
    if state.session_id != session_id || state.lane != LANE_INCOMING {
        return IncomingTimedWait::Stopped;
    }
    match state.pending.pop_front() {
        Some(utterance) => IncomingTimedWait::Utterance(utterance),
        None => IncomingTimedWait::TimedOut,
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
    gate: &VadGateResult,
) -> bool {
    let speech_like = gate.accepted;
    if !state.in_utterance {
        if speech_like {
            state.in_utterance = true;
            state.current_input_start = state.input_sample_cursor
                .checked_sub(samples.len() as u64)
                .and_then(|start| start.checked_sub(state.pre_roll.len() as u64));
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
        let max_samples = samples_for_duration(
            state.sample_rate_hz, MAX_IN_PROGRESS_UTTERANCE_MS,
        );
        // The additional checks matter only for rare near-limit utterances,
        // not for every ordinary audio callback.
        if state.current_samples.len() > max_samples {
            // A phrase just under 60 s must not be discarded merely because
            // its closing VAD-rejected quiet frame makes the buffer longer.
            let closing_quiet = !speech_like
                && gate.reason != "rejected_clipping"
                && state.speech_samples < max_samples
                && state.current_samples.len()
                    <= samples_for_duration(
                        state.sample_rate_hz,
                        MAX_IN_PROGRESS_UTTERANCE_MS + MAX_FINALIZING_QUIET_OVERHANG_MS,
                    );
            if !closing_quiet {
                // Never invent a cut during continuous speech. The genuinely
                // overlong utterance still fails closed until a natural reset.
                state.current_samples.clear();
                state.overflowed = true;
            }
        }
    }

    if speech_like {
        state.speech_samples = state.speech_samples.saturating_add(samples.len());
        state.trailing_silence_samples = 0;
        return false;
    }

    // Clipping is a quality rejection, not evidence of silence. A distorted
    // burst inside speech must not end the utterance prematurely; preserve
    // the bounded samples for the unchanged final whole-utterance quality gate.
    // Clipping alone still cannot start a new utterance or count as speech.
    if gate.reason == "rejected_clipping" {
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
    let max_source_samples =
        samples_for_duration(state.sample_rate_hz, MAX_IN_PROGRESS_UTTERANCE_MS);
    if speech_end > max_source_samples {
        // Only silence may be trimmed. Never pass a truncated spoken phrase
        // to ASR or count it as successfully translated.
        OVERFLOW_DROPPED_UTTERANCES.fetch_add(1, Ordering::Relaxed);
        reset_current_utterance(state);
        return false;
    }

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

    let captured_samples = state.current_samples.len();
    let mut source_samples = std::mem::take(&mut state.current_samples);
    // The only permitted truncation is excess *trailing silence* from the
    // bounded finalization overhang, never recognized speech or pre-roll.
    source_samples.truncate(max_source_samples);
    // Bind provenance to the actual retained WAV samples, excluding any
    // discarded quiet suffix. Verify continuity against the full captured
    // callback range before shortening the endpoint.
    let native_range = state.current_input_start.and_then(|start| {
        (start.checked_add(captured_samples as u64) == Some(state.input_sample_cursor))
            .then_some((start, start + source_samples.len() as u64))
    });
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
        capture_provenance: native_range.map(|(start, end)| FinalizedCaptureProvenance {
            audio_epoch: state.capture_epoch,
            input_rate_hz: state.sample_rate_hz,
            input_start_sample: start,
            input_end_sample: end,
            asr_sample_count: target_samples.len() as u64,
        }),
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
    let max_samples = samples_for_duration(state.sample_rate_hz, state.profile.pre_roll_audio_ms);
    if max_samples == 0 {
        state.pre_roll.clear();
        return;
    }
    // Evict BEFORE appending so a fully preallocated circular VecDeque never
    // grows just to discard the oldest samples inside an audio callback.
    // A callback larger than the entire pre-roll keeps only its newest tail.
    let recent = &samples[samples.len().saturating_sub(max_samples)..];
    for &sample in recent {
        if state.pre_roll.len() >= max_samples {
            let _ = state.pre_roll.pop_front();
        }
        state.pre_roll.push_back(safe_sample(sample));
    }
}

fn extend_safe_samples(target: &mut Vec<f32>, samples: &[f32]) {
    target.extend(samples.iter().map(|sample| safe_sample(*sample)));
}

fn reset_current_utterance(state: &mut FinalizedProducerState) {
    state.in_utterance = false;
    state.current_input_start = None;
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
#[path = "finalized_utterance_tests.rs"]
mod tests;
