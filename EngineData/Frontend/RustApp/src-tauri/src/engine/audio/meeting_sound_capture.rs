use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::device_selection::resolve_unique_named_device;
use super::finalized_utterance::{
    clear_finalized_incoming_utterance_producer, observe_finalized_incoming_f32_samples,
    observe_finalized_incoming_i16_samples, observe_finalized_incoming_i32_samples,
    observe_finalized_incoming_i64_samples, observe_finalized_incoming_u8_samples,
    reset_finalized_incoming_speech_boundary, reset_finalized_incoming_utterance_producer,
};
use crate::engine::runtime_settings::load_settings;

#[derive(Debug, Clone, Serialize)]
pub struct MeetingSoundCaptureStatus {
    pub stream_active: bool,
    pub session_id: Option<String>,
    pub device_name: Option<String>,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u16>,
    pub sample_format: Option<String>,
    pub started_unix_ms: Option<u128>,
    pub active_age_ms: Option<u128>,
    pub frames_received: u64,
    pub suppressed_frames: u64,
    pub suppression_active: bool,
    pub callback_error_count: usize,
    pub latest_callback_error: Option<String>,
    pub blocker: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingSoundCaptureStartReport {
    pub ok: bool,
    pub status: MeetingSoundCaptureStatus,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingSoundCaptureStopReport {
    pub ok: bool,
    pub status: MeetingSoundCaptureStatus,
    pub message: String,
}

struct MeetingSoundCaptureRuntime {
    stop_tx: mpsc::Sender<()>,
    capture_thread: Option<thread::JoinHandle<()>>,
    session_id: String,
    device_name: Option<String>,
    sample_rate_hz: u32,
    channels: u16,
    sample_format: String,
    started_unix_ms: u128,
    frames_received: Arc<AtomicU64>,
    suppressed_frames: Arc<AtomicU64>,
    suppression: Arc<AtomicBool>,
    callback_errors: Arc<Mutex<Vec<String>>>,
}

struct MeetingSoundReady {
    device_name: Option<String>,
    sample_rate_hz: u32,
    channels: u16,
    sample_format: String,
    started_unix_ms: u128,
}

static MEETING_SOUND_CAPTURE_RUNTIME: OnceLock<Mutex<Option<MeetingSoundCaptureRuntime>>> =
    OnceLock::new();

pub fn start_meeting_sound_capture_runtime(
    session_id: &str,
    suppression: Arc<AtomicBool>,
) -> MeetingSoundCaptureStartReport {
    if !cfg!(target_os = "windows") {
        return blocked_start(
            "meeting_sound:windows_loopback_required",
            "Incoming Meeting Sound capture is available only on the supported Windows runtime.",
        );
    }

    let session_id = session_id.trim().to_string();
    if session_id.is_empty() {
        return blocked_start(
            "meeting_sound:missing_session_id",
            "Meeting Sound capture requires the current application Meeting session identity.",
        );
    }

    let store = MEETING_SOUND_CAPTURE_RUNTIME.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = store.lock() else {
        return blocked_start(
            "meeting_sound:state_lock_failed",
            "Meeting Sound capture state is unavailable.",
        );
    };

    if let Some(runtime) = guard.as_ref() {
        let status = build_status_from_guard(Some(runtime));
        return MeetingSoundCaptureStartReport {
            ok: runtime.session_id == session_id,
            status,
            message: if runtime.session_id == session_id {
                "Meeting Sound loopback capture is already active for this Meeting session."
                    .to_string()
            } else {
                "Another Meeting Sound capture session is already active.".to_string()
            },
        };
    }

    let frames_received = Arc::new(AtomicU64::new(0));
    let suppressed_frames = Arc::new(AtomicU64::new(0));
    let callback_errors = Arc::new(Mutex::new(Vec::new()));
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let (stop_tx, stop_rx) = mpsc::channel();

    let thread_session_id = session_id.clone();
    let thread_frames = Arc::clone(&frames_received);
    let thread_suppressed = Arc::clone(&suppressed_frames);
    let thread_suppression = Arc::clone(&suppression);
    let thread_errors = Arc::clone(&callback_errors);
    let capture_thread = thread::spawn(move || {
        if let Err(error) = run_capture_thread(
            &thread_session_id,
            thread_frames,
            thread_suppressed,
            thread_suppression,
            thread_errors,
            stop_rx,
            &ready_tx,
        ) {
            let _ = ready_tx.send(Err(error));
        }
    });

    let ready = match ready_rx.recv_timeout(Duration::from_secs(10)) {
        Ok(Ok(ready)) => ready,
        Ok(Err(error)) => {
            let _ = capture_thread.join();
            clear_finalized_incoming_utterance_producer();
            return blocked_start("meeting_sound:loopback_start_failed", &error);
        }
        Err(error) => {
            let _ = stop_tx.send(());
            let _ = capture_thread.join();
            clear_finalized_incoming_utterance_producer();
            return blocked_start(
                "meeting_sound:loopback_start_timeout",
                &format!("Timed out while opening Meeting Sound loopback capture: {error}"),
            );
        }
    };

    *guard = Some(MeetingSoundCaptureRuntime {
        stop_tx,
        capture_thread: Some(capture_thread),
        session_id,
        device_name: ready.device_name,
        sample_rate_hz: ready.sample_rate_hz,
        channels: ready.channels,
        sample_format: ready.sample_format,
        started_unix_ms: ready.started_unix_ms,
        frames_received,
        suppressed_frames,
        suppression,
        callback_errors,
    });

    MeetingSoundCaptureStartReport {
        ok: true,
        status: build_status_from_guard(guard.as_ref()),
        message: "Meeting Sound output-loopback capture started for the current Meeting session. Incoming audio remains an optional lane and only finalized speech may enter incoming ASR."
            .to_string(),
    }
}

pub fn stop_meeting_sound_capture_runtime() -> MeetingSoundCaptureStopReport {
    let store = MEETING_SOUND_CAPTURE_RUNTIME.get_or_init(|| Mutex::new(None));
    let runtime = match store.lock() {
        Ok(mut guard) => guard.take(),
        Err(_) => {
            clear_finalized_incoming_utterance_producer();
            return MeetingSoundCaptureStopReport {
                ok: false,
                status: inactive_status(
                    "meeting_sound:state_lock_failed",
                    "Meeting Sound capture state lock failed while stopping.",
                ),
                message: "Meeting Sound capture could not acquire its state lock during Stop."
                    .to_string(),
            };
        }
    };

    let Some(mut runtime) = runtime else {
        clear_finalized_incoming_utterance_producer();
        return MeetingSoundCaptureStopReport {
            ok: true,
            status: inactive_status("meeting_sound:not_active", "Meeting Sound capture is not active."),
            message: "No Meeting Sound capture stream required cleanup.".to_string(),
        };
    };

    let _ = runtime.stop_tx.send(());
    let joined = runtime
        .capture_thread
        .take()
        .map(|handle| handle.join().is_ok())
        .unwrap_or(true);
    clear_finalized_incoming_utterance_producer();

    MeetingSoundCaptureStopReport {
        ok: joined,
        status: inactive_status("meeting_sound:stopped", "Meeting Sound capture ownership was released."),
        message: if joined {
            "Meeting Sound loopback capture stopped and pending incoming audio state was cleared."
                .to_string()
        } else {
            "Meeting Sound loopback capture released its state, but the capture thread exited unexpectedly."
                .to_string()
        },
    }
}

pub fn meeting_sound_capture_status() -> MeetingSoundCaptureStatus {
    let store = MEETING_SOUND_CAPTURE_RUNTIME.get_or_init(|| Mutex::new(None));
    match store.lock() {
        Ok(guard) => build_status_from_guard(guard.as_ref()),
        Err(_) => inactive_status(
            "meeting_sound:state_lock_failed",
            "Meeting Sound capture status is temporarily unavailable.",
        ),
    }
}

fn run_capture_thread(
    session_id: &str,
    frames_received: Arc<AtomicU64>,
    suppressed_frames: Arc<AtomicU64>,
    suppression: Arc<AtomicBool>,
    callback_errors: Arc<Mutex<Vec<String>>>,
    stop_rx: mpsc::Receiver<()>,
    ready_tx: &mpsc::SyncSender<Result<MeetingSoundReady, String>>,
) -> Result<(), String> {
    let host = cpal::default_host();
    let device = select_output_device(&host)?;
    let device_name = device.name().ok();
    let supported = device
        .default_output_config()
        .map_err(|error| format!("Selected Meeting Sound endpoint has no usable output mix config: {error}"))?;
    let sample_format = supported.sample_format();
    let stream_config = supported.config();
    let sample_rate_hz = stream_config.sample_rate.0;
    let channels = stream_config.channels;

    reset_finalized_incoming_utterance_producer(session_id, sample_rate_hz);
    let stream = build_loopback_stream(
        &device,
        &stream_config,
        sample_format,
        frames_received,
        suppressed_frames,
        suppression,
        callback_errors,
    )?;
    stream
        .play()
        .map_err(|error| format!("Failed to start Meeting Sound WASAPI loopback stream: {error}"))?;

    ready_tx
        .send(Ok(MeetingSoundReady {
            device_name,
            sample_rate_hz,
            channels,
            sample_format: format!("{sample_format:?}"),
            started_unix_ms: current_unix_ms(),
        }))
        .map_err(|error| format!("Failed to report Meeting Sound loopback readiness: {error}"))?;

    let _ = stop_rx.recv();
    drop(stream);
    clear_finalized_incoming_utterance_producer();
    Ok(())
}

fn build_loopback_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    frames_received: Arc<AtomicU64>,
    suppressed_frames: Arc<AtomicU64>,
    suppression: Arc<AtomicBool>,
    callback_errors: Arc<Mutex<Vec<String>>>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels;
    let sample_rate_hz = config.sample_rate.0;

    macro_rules! build_stream {
        ($sample:ty, $observe:path) => {{
            let frames = Arc::clone(&frames_received);
            let suppressed = Arc::clone(&suppressed_frames);
            let suppression_flag = Arc::clone(&suppression);
            let errors = Arc::clone(&callback_errors);
            device
                .build_input_stream(
                    config,
                    move |data: &[$sample], _| {
                        let frame_count = frame_count(data.len(), channels);
                        frames.fetch_add(frame_count, Ordering::Relaxed);
                        if suppression_flag.load(Ordering::Acquire) {
                            suppressed.fetch_add(frame_count, Ordering::Relaxed);
                            reset_finalized_incoming_speech_boundary();
                            return;
                        }
                        $observe(data, sample_rate_hz, channels);
                    },
                    move |error| push_callback_error(&errors, error),
                    None,
                )
                .map_err(|error| error.to_string())
        }};
    }

    match sample_format {
        cpal::SampleFormat::F32 => build_stream!(f32, observe_finalized_incoming_f32_samples),
        cpal::SampleFormat::I16 => build_stream!(i16, observe_finalized_incoming_i16_samples),
        cpal::SampleFormat::I32 => build_stream!(i32, observe_finalized_incoming_i32_samples),
        cpal::SampleFormat::I64 => build_stream!(i64, observe_finalized_incoming_i64_samples),
        cpal::SampleFormat::U8 => build_stream!(u8, observe_finalized_incoming_u8_samples),
        other => Err(format!(
            "Selected Meeting Sound output mix format is not supported by the current loopback conversion boundary: {other:?}"
        )),
    }
}

fn select_output_device(host: &cpal::Host) -> Result<cpal::Device, String> {
    let configured = load_settings()
        .audio
        .output_device_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if let Some(requested_name) = configured {
        let devices = host
            .output_devices()
            .map_err(|error| format!("Meeting Sound endpoints could not be enumerated: {error}"))?;
        let resolved = resolve_unique_named_device(devices, &requested_name, |device| device.name().ok())
            .map_err(|_| format!(
                "Configured Meeting Sound endpoint '{requested_name}' is ambiguous. Incoming will not bind to an arbitrary output."
            ))?;
        return resolved.ok_or_else(|| format!(
            "Configured Meeting Sound endpoint '{requested_name}' is unavailable. TranslateIT will not silently switch to another output endpoint."
        ));
    }

    host.default_output_device()
        .ok_or_else(|| "No Windows default Meeting Sound output endpoint was found.".to_string())
}

fn build_status_from_guard(runtime: Option<&MeetingSoundCaptureRuntime>) -> MeetingSoundCaptureStatus {
    match runtime {
        Some(runtime) => {
            let errors = runtime
                .callback_errors
                .lock()
                .ok()
                .map(|guard| guard.clone())
                .unwrap_or_default();
            MeetingSoundCaptureStatus {
                stream_active: true,
                session_id: Some(runtime.session_id.clone()),
                device_name: runtime.device_name.clone(),
                sample_rate_hz: Some(runtime.sample_rate_hz),
                channels: Some(runtime.channels),
                sample_format: Some(runtime.sample_format.clone()),
                started_unix_ms: Some(runtime.started_unix_ms),
                active_age_ms: Some(current_unix_ms().saturating_sub(runtime.started_unix_ms)),
                frames_received: runtime.frames_received.load(Ordering::Relaxed),
                suppressed_frames: runtime.suppressed_frames.load(Ordering::Relaxed),
                suppression_active: runtime.suppression.load(Ordering::Acquire),
                callback_error_count: errors.len(),
                latest_callback_error: errors.last().cloned(),
                blocker: String::new(),
                note: "Meeting Sound capture owns one Windows output-loopback stream for the current Meeting session. Actual device audio and suppression behavior require Windows runtime proof."
                    .to_string(),
            }
        }
        None => inactive_status("meeting_sound:not_active", "Meeting Sound capture is not active."),
    }
}

fn blocked_start(blocker: &str, message: &str) -> MeetingSoundCaptureStartReport {
    MeetingSoundCaptureStartReport {
        ok: false,
        status: inactive_status(blocker, message),
        message: message.to_string(),
    }
}

fn inactive_status(blocker: &str, note: &str) -> MeetingSoundCaptureStatus {
    MeetingSoundCaptureStatus {
        stream_active: false,
        session_id: None,
        device_name: None,
        sample_rate_hz: None,
        channels: None,
        sample_format: None,
        started_unix_ms: None,
        active_age_ms: None,
        frames_received: 0,
        suppressed_frames: 0,
        suppression_active: false,
        callback_error_count: 0,
        latest_callback_error: None,
        blocker: blocker.to_string(),
        note: note.to_string(),
    }
}

fn frame_count(sample_count: usize, channels: u16) -> u64 {
    (sample_count / usize::from(channels.max(1))) as u64
}

fn push_callback_error(callback_errors: &Arc<Mutex<Vec<String>>>, error: cpal::StreamError) {
    if let Ok(mut errors) = callback_errors.lock() {
        errors.push(error.to_string());
        if errors.len() > 20 {
            errors.remove(0);
        }
    }
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
