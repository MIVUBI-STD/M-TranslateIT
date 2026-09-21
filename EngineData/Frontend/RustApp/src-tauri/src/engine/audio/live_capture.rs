use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::finalized_utterance::{
    clear_finalized_outbound_utterance_producer, observe_finalized_outbound_f32_samples,
    observe_finalized_outbound_mono_f32_samples, reset_finalized_outbound_utterance_producer,
};
use super::guided_take::append_guided_f32;
use super::live_audio_buffer::{
    append_live_f32_samples, append_live_mono_f32_samples, clear_live_audio_buffer,
    reset_live_audio_buffer,
};
use crate::engine::runtime_settings::load_settings;
use crate::engine::runtime_state::RuntimeSessionStateReport;

const APPLICATION_MEETING_CAPTURE_OWNER_ID: &str = "translateit_application_meeting";

#[derive(Debug, Clone, Serialize)]
pub struct LiveCaptureStatusReport {
    pub stream_active: bool,
    pub owner_id: Option<String>,
    pub session_id: Option<String>,
    pub device_name: Option<String>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u16>,
    pub sample_format: Option<String>,
    pub started_unix_ms: Option<u128>,
    pub active_age_ms: Option<u128>,
    pub frames_received: u64,
    pub callback_error_count: usize,
    pub latest_callback_error: Option<String>,
    pub blocker: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveCaptureStartReport {
    pub ok: bool,
    pub status: LiveCaptureStatusReport,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveCaptureStopReport {
    pub ok: bool,
    pub status: LiveCaptureStatusReport,
    pub message: String,
}

struct LiveCaptureRuntime {
    stop_tx: mpsc::Sender<()>,
    capture_thread: Option<thread::JoinHandle<()>>,
    owner_id: String,
    session_id: String,
    device_name: Option<String>,
    sample_rate_hz: u32,
    channels: u16,
    sample_format: String,
    started_unix_ms: u128,
    frames_received: Arc<AtomicU64>,
    callback_errors: Arc<Mutex<Vec<String>>>,
}

static LIVE_CAPTURE_RUNTIME: OnceLock<Mutex<Option<LiveCaptureRuntime>>> = OnceLock::new();

struct LiveCaptureReady {
    device_name: Option<String>,
    sample_rate_hz: u32,
    channels: u16,
    sample_format: String,
    started_unix_ms: u128,
}

pub fn start_live_capture_runtime(
    session_state: RuntimeSessionStateReport,
) -> LiveCaptureStartReport {
    let Some(session) = session_state.snapshot.as_ref() else {
        return blocked_start(
            "live_capture:no_active_session",
            "Live capture cannot start because no runtime session is active.",
        );
    };

    if !session.safe_to_stop {
        return blocked_start(
            "live_capture:session_not_safe",
            "Live capture cannot start because the active runtime session is not safe to stop.",
        );
    }

    let store = LIVE_CAPTURE_RUNTIME.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = store.lock() else {
        return blocked_start(
            "live_capture:state_lock_failed",
            "Live capture state lock failed before stream creation.",
        );
    };

    if guard.is_some() {
        let status = build_status_from_guard(guard.as_ref());
        return LiveCaptureStartReport {
            ok: false,
            status,
            message: "Live microphone stream is already active. Stop it before starting a new capture stream.".to_string(),
        };
    }

    let frames_received = Arc::new(AtomicU64::new(0));
    let callback_errors = Arc::new(Mutex::new(Vec::new()));
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let (stop_tx, stop_rx) = mpsc::channel();
    let thread_frames = Arc::clone(&frames_received);
    let thread_errors = Arc::clone(&callback_errors);
    let capture_session_id = session.session_id.clone();
    let capture_generation = session.generation;
    let finalized_outbound_enabled = session.owner_id == APPLICATION_MEETING_CAPTURE_OWNER_ID;
    let capture_thread = thread::spawn(move || {
        if let Err(error) = run_capture_thread(
            thread_frames,
            thread_errors,
            stop_rx,
            &ready_tx,
            capture_session_id,
            capture_generation,
            finalized_outbound_enabled,
        ) {
            let _ = ready_tx.send(Err(error));
        }
    });

    let ready = match ready_rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(ready)) => ready,
        Ok(Err(error)) => {
            let _ = capture_thread.join();
            clear_live_audio_buffer();
            clear_finalized_outbound_utterance_producer();
            return blocked_start("live_capture:stream_build_failed", &error);
        }
        Err(error) => {
            let _ = stop_tx.send(());
            let _ = capture_thread.join();
            clear_live_audio_buffer();
            clear_finalized_outbound_utterance_producer();
            return blocked_start(
                "live_capture:stream_start_timeout",
                &format!("Timed out while starting live microphone stream: {error}"),
            );
        }
    };

    let runtime = LiveCaptureRuntime {
        stop_tx,
        capture_thread: Some(capture_thread),
        owner_id: session.owner_id.clone(),
        session_id: session.session_id.clone(),
        device_name: ready.device_name,
        sample_rate_hz: ready.sample_rate_hz,
        channels: ready.channels,
        sample_format: ready.sample_format,
        started_unix_ms: ready.started_unix_ms,
        frames_received,
        callback_errors,
    };

    *guard = Some(runtime);
    let status = build_status_from_guard(guard.as_ref());

    LiveCaptureStartReport {
        ok: true,
        status,
        message: if finalized_outbound_enabled {
            "Live microphone stream started for the authoritative Meeting session. Audio feeds both the rolling preview buffer and the audio-owned finalized-utterance producer; only finalized utterances may enter outbound AI stages."
                .to_string()
        } else {
            "Live microphone stream started for a capture-only runtime owner. Audio feeds the rolling diagnostic buffer; Meeting finalized-output production is not active for this owner."
                .to_string()
        },
    }
}

pub fn live_capture_status() -> LiveCaptureStatusReport {
    let store = LIVE_CAPTURE_RUNTIME.get_or_init(|| Mutex::new(None));
    match store.lock() {
        Ok(guard) => build_status_from_guard(guard.as_ref()),
        Err(_) => inactive_status(
            "live_capture:state_lock_failed",
            "Live microphone capture status is temporarily unavailable.",
        ),
    }
}

pub fn stop_live_capture_runtime() -> LiveCaptureStopReport {
    let store = LIVE_CAPTURE_RUNTIME.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = store.lock() else {
        return LiveCaptureStopReport {
            ok: false,
            status: inactive_status(
                "live_capture:state_lock_failed",
                "Live capture state lock failed while stopping.",
            ),
            message: "Live capture stop failed because state lock could not be acquired."
                .to_string(),
        };
    };

    let Some(mut runtime) = guard.take() else {
        clear_live_audio_buffer();
        clear_finalized_outbound_utterance_producer();
        return LiveCaptureStopReport {
            ok: true,
            status: inactive_status(
                "live_capture:not_active",
                "No live microphone stream was active.",
            ),
            message: "No live microphone stream was active. Rolling/finalized audio state was cleared and Stop remains safe."
                .to_string(),
        };
    };

    drop(guard);
    let _ = runtime.stop_tx.send(());
    let thread_stopped = runtime
        .capture_thread
        .take()
        .map(|handle| handle.join().is_ok())
        .unwrap_or(true);
    clear_live_audio_buffer();
    clear_finalized_outbound_utterance_producer();
    LiveCaptureStopReport {
        ok: thread_stopped,
        status: inactive_status(
            "live_capture:stopped",
            "Live microphone stream ownership was released.",
        ),
        message: if thread_stopped {
            "Live microphone stream stopped; rolling and pending finalized audio were cleared and ownership was released."
                .to_string()
        } else {
            "Live microphone stream stopped and audio state was cleared, but its owner thread exited unexpectedly."
                .to_string()
        },
    }
}

fn ensure_scratch_capacity<T>(samples: &[T], source_channels: u16, output: &mut Vec<f32>) {
    let channel_count = usize::from(source_channels.max(1));
    let required = samples.len().div_ceil(channel_count);
    if output.capacity() < required {
        output.reserve(required);
    }
}

fn downmix_f32_into(samples: &[f32], source_channels: u16, output: &mut Vec<f32>) {
    output.clear();
    ensure_scratch_capacity(samples, source_channels, output);
    let channel_count = usize::from(source_channels.max(1));
    for frame in samples.chunks(channel_count) {
        let sum = frame.iter().map(|sample| safe_sample(*sample)).sum::<f32>();
        output.push(sum / frame.len().max(1) as f32);
    }
}

fn mono_f32_from_i16_into(samples: &[i16], source_channels: u16, output: &mut Vec<f32>) {
    output.clear();
    ensure_scratch_capacity(samples, source_channels, output);
    let channel_count = usize::from(source_channels.max(1));
    for frame in samples.chunks(channel_count) {
        let sum = frame
            .iter()
            .map(|sample| (*sample as f32 / i16::MAX as f32).clamp(-1.0, 1.0))
            .sum::<f32>();
        output.push(sum / frame.len().max(1) as f32);
    }
}

fn mono_f32_from_u16_into(samples: &[u16], source_channels: u16, output: &mut Vec<f32>) {
    output.clear();
    ensure_scratch_capacity(samples, source_channels, output);
    let channel_count = usize::from(source_channels.max(1));
    for frame in samples.chunks(channel_count) {
        let sum = frame
            .iter()
            .map(|sample| ((*sample as f32 / u16::MAX as f32) * 2.0 - 1.0).clamp(-1.0, 1.0))
            .sum::<f32>();
        output.push(sum / frame.len().max(1) as f32);
    }
}

fn publish_runtime_mono(
    samples_mono: &[f32],
    sample_rate_hz: u32,
    source_channels: u16,
    finalized_outbound_enabled: bool,
) {
    append_live_mono_f32_samples(samples_mono, sample_rate_hz, source_channels);
    if finalized_outbound_enabled {
        observe_finalized_outbound_mono_f32_samples(samples_mono, sample_rate_hz);
    }
}

fn publish_runtime_native_f32_mono(
    samples_mono: &[f32],
    sample_rate_hz: u32,
    finalized_outbound_enabled: bool,
) {
    // Keep the generic public wrappers alive on the native mono-F32 path. Their
    // single-channel branches forward directly to the specialized mono owners, so
    // this preserves the zero-downmix-allocation fast path while avoiding orphaned
    // source that the repository's -D dead_code compiler contract must reject.
    append_live_f32_samples(samples_mono, sample_rate_hz, 1);
    if finalized_outbound_enabled {
        observe_finalized_outbound_f32_samples(samples_mono, sample_rate_hz, 1);
    }
}

fn build_stream_for_format(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    frames_received: Arc<AtomicU64>,
    callback_errors: Arc<Mutex<Vec<String>>>,
    finalized_outbound_enabled: bool,
) -> Result<cpal::Stream, String> {
    let channels = config.channels;
    let sample_rate_hz = config.sample_rate.0;

    match sample_format {
        cpal::SampleFormat::F32 => {
            let frames = Arc::clone(&frames_received);
            let errors = Arc::clone(&callback_errors);
            let mut mono_scratch = Vec::<f32>::new();
            device
                .build_input_stream(
                    config,
                    move |data: &[f32], _| {
                        record_frames(data.len(), channels, &frames);
                        if channels <= 1 {
                            // The common mono-F32 path needs no conversion buffer.
                            publish_runtime_native_f32_mono(
                                data,
                                sample_rate_hz,
                                finalized_outbound_enabled,
                            );
                        } else {
                            downmix_f32_into(data, channels, &mut mono_scratch);
                            publish_runtime_mono(
                                &mono_scratch,
                                sample_rate_hz,
                                channels,
                                finalized_outbound_enabled,
                            );
                        }
                        // The guided recorder is normally inactive during Meeting;
                        // its atomic gate returns before sample work when no guided
                        // take owns capture state.
                        append_guided_f32(data, sample_rate_hz, channels);
                    },
                    move |error| push_callback_error(&errors, error),
                    None,
                )
                .map_err(|error| error.to_string())
        }
        cpal::SampleFormat::I16 => {
            let frames = Arc::clone(&frames_received);
            let errors = Arc::clone(&callback_errors);
            let mut mono_scratch = Vec::<f32>::new();
            device
                .build_input_stream(
                    config,
                    move |data: &[i16], _| {
                        record_frames(data.len(), channels, &frames);
                        // Reuse one callback-owned scratch Vec. Conversion and downmix
                        // happen in one pass instead of allocating a converted Vec and
                        // then allocating another mono Vec for every callback.
                        mono_f32_from_i16_into(data, channels, &mut mono_scratch);
                        publish_runtime_mono(
                            &mono_scratch,
                            sample_rate_hz,
                            channels,
                            finalized_outbound_enabled,
                        );
                        append_guided_f32(&mono_scratch, sample_rate_hz, 1);
                    },
                    move |error| push_callback_error(&errors, error),
                    None,
                )
                .map_err(|error| error.to_string())
        }
        cpal::SampleFormat::U16 => {
            let frames = Arc::clone(&frames_received);
            let errors = Arc::clone(&callback_errors);
            let mut mono_scratch = Vec::<f32>::new();
            device
                .build_input_stream(
                    config,
                    move |data: &[u16], _| {
                        record_frames(data.len(), channels, &frames);
                        mono_f32_from_u16_into(data, channels, &mut mono_scratch);
                        publish_runtime_mono(
                            &mono_scratch,
                            sample_rate_hz,
                            channels,
                            finalized_outbound_enabled,
                        );
                        append_guided_f32(&mono_scratch, sample_rate_hz, 1);
                    },
                    move |error| push_callback_error(&errors, error),
                    None,
                )
                .map_err(|error| error.to_string())
        }
        other => Err(format!(
            "Unsupported default microphone sample format: {other:?}"
        )),
    }
}

fn configured_input_device_name() -> Option<String> {
    load_settings()
        .audio
        .input_device_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn select_input_device(host: &cpal::Host) -> Result<cpal::Device, String> {
    if let Some(requested_name) = configured_input_device_name() {
        let devices = host
            .input_devices()
            .map_err(|error| format!("Configured microphone could not be enumerated: {error}"))?;
        for device in devices {
            if device.name().ok().as_deref() == Some(requested_name.as_str()) {
                return Ok(device);
            }
        }
        return Err(format!(
            "Configured microphone '{requested_name}' is unavailable. TranslateIT will not silently switch to another microphone."
        ));
    }

    host.default_input_device()
        .ok_or_else(|| "No default microphone input device was found.".to_string())
}

fn run_capture_thread(
    frames_received: Arc<AtomicU64>,
    callback_errors: Arc<Mutex<Vec<String>>>,
    stop_rx: mpsc::Receiver<()>,
    ready_tx: &mpsc::SyncSender<Result<LiveCaptureReady, String>>,
    session_id: String,
    generation: u64,
    finalized_outbound_enabled: bool,
) -> Result<(), String> {
    let host = cpal::default_host();
    let device = select_input_device(&host)?;
    let device_name = device.name().ok();
    let default_config = device
        .default_input_config()
        .map_err(|error| format!("Selected microphone has no usable input config: {error}"))?;
    let sample_format = default_config.sample_format();
    let stream_config: cpal::StreamConfig = default_config.into();
    let sample_rate_hz = stream_config.sample_rate.0;
    let channels = stream_config.channels;

    reset_live_audio_buffer(sample_rate_hz, channels);
    if finalized_outbound_enabled {
        reset_finalized_outbound_utterance_producer(&session_id, generation, sample_rate_hz);
    } else {
        clear_finalized_outbound_utterance_producer();
    }
    let stream = match build_stream_for_format(
        &device,
        &stream_config,
        sample_format,
        frames_received,
        callback_errors,
        finalized_outbound_enabled,
    ) {
        Ok(stream) => stream,
        Err(error) => {
            clear_finalized_outbound_utterance_producer();
            return Err(format!("Failed to build live microphone stream: {error}"));
        }
    };
    if let Err(error) = stream.play() {
        clear_finalized_outbound_utterance_producer();
        return Err(format!("Failed to start live microphone stream: {error}"));
    }

    if let Err(error) = ready_tx.send(Ok(LiveCaptureReady {
        device_name,
        sample_rate_hz,
        channels,
        sample_format: format!("{sample_format:?}"),
        started_unix_ms: current_unix_ms(),
    })) {
        drop(stream);
        clear_finalized_outbound_utterance_producer();
        return Err(format!("Failed to report live microphone readiness: {error}"));
    }

    // The stream must stay on its owner thread because cpal streams are not Send on all platforms.
    let _ = stop_rx.recv();
    drop(stream);
    clear_finalized_outbound_utterance_producer();
    Ok(())
}

fn record_frames(sample_count: usize, channels: u16, frames_received: &Arc<AtomicU64>) {
    let channel_count = usize::from(channels.max(1));
    frames_received.fetch_add((sample_count / channel_count) as u64, Ordering::Relaxed);
}

fn push_callback_error(callback_errors: &Arc<Mutex<Vec<String>>>, error: cpal::StreamError) {
    if let Ok(mut errors) = callback_errors.lock() {
        errors.push(error.to_string());
        if errors.len() > 20 {
            errors.remove(0);
        }
    }
}

fn build_status_from_guard(runtime: Option<&LiveCaptureRuntime>) -> LiveCaptureStatusReport {
    match runtime {
        Some(runtime) => {
            let active_age_ms = current_unix_ms().saturating_sub(runtime.started_unix_ms);
            let errors = runtime
                .callback_errors
                .lock()
                .ok()
                .map(|guard| guard.clone())
                .unwrap_or_default();
            LiveCaptureStatusReport {
                stream_active: true,
                owner_id: Some(runtime.owner_id.clone()),
                session_id: Some(runtime.session_id.clone()),
                device_name: runtime.device_name.clone(),
                sample_rate_hz: Some(runtime.sample_rate_hz),
                channels: Some(runtime.channels),
                sample_format: Some(runtime.sample_format.clone()),
                started_unix_ms: Some(runtime.started_unix_ms),
                active_age_ms: Some(active_age_ms),
                frames_received: runtime.frames_received.load(Ordering::Relaxed),
                callback_error_count: errors.len(),
                latest_callback_error: errors.last().cloned(),
                blocker: String::new(),
                note: format!(
                    "Live microphone stream is active. age_ms={}, sample_rate_hz={}, channels={}. Rolling audio remains preview-capable; finalized output production is scoped to the application Meeting owner.",
                    active_age_ms, runtime.sample_rate_hz, runtime.channels
                ),
            }
        }
        None => inactive_status(
            "live_capture:not_active",
            "Live microphone stream is not active.",
        ),
    }
}

fn blocked_start(blocker: &str, message: &str) -> LiveCaptureStartReport {
    LiveCaptureStartReport {
        ok: false,
        status: inactive_status(blocker, message),
        message: message.to_string(),
    }
}

fn inactive_status(blocker: &str, note: &str) -> LiveCaptureStatusReport {
    LiveCaptureStatusReport {
        stream_active: false,
        owner_id: None,
        session_id: None,
        device_name: None,
        sample_rate_hz: None,
        channels: None,
        sample_format: None,
        started_unix_ms: None,
        active_age_ms: None,
        frames_received: 0,
        callback_error_count: 0,
        latest_callback_error: None,
        blocker: blocker.to_string(),
        note: note.to_string(),
    }
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
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

    #[test]
    fn callback_conversion_reuses_scratch_and_downmixes_in_one_pass() {
        let mut scratch = Vec::new();
        let stereo_i16 = [i16::MAX, 0, i16::MIN, 0];
        mono_f32_from_i16_into(&stereo_i16, 2, &mut scratch);
        assert_eq!(scratch.len(), 2);
        assert!(scratch[0] > 0.49 && scratch[0] <= 0.5);
        assert!(scratch[1] >= -0.51 && scratch[1] < -0.49);
        let first_capacity = scratch.capacity();

        mono_f32_from_i16_into(&[0, 0], 2, &mut scratch);
        assert_eq!(scratch, vec![0.0]);
        assert!(scratch.capacity() >= first_capacity);

        mono_f32_from_u16_into(&[0, u16::MAX], 2, &mut scratch);
        assert_eq!(scratch.len(), 1);
        assert!(scratch[0].abs() <= 1e-6);

        downmix_f32_into(&[f32::NAN, 1.0, 1.0, -1.0], 2, &mut scratch);
        assert_eq!(scratch, vec![0.5, 0.0]);
    }
}
