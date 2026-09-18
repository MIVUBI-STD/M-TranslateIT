use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::engine::runtime_state::runtime_generation_is_authoritative;

#[path = "meeting_output_runtime/audio_format.rs"]
mod audio_format;

use audio_format::{decode_wav_file, delivery_deadline_ms, prepare_output_samples};
#[cfg(test)]
use audio_format::decode_wav_bytes;

const MAX_MEETING_OUTPUT_WAV_BYTES: u64 = 32 * 1024 * 1024;
const MAX_OUTPUT_CHANNELS: u16 = 8;
const MAX_OUTPUT_SAMPLE_RATE_HZ: u32 = 192_000;
const MIN_OUTPUT_SAMPLE_RATE_HZ: u32 = 8_000;
const FUNCTIONAL_OUTPUT_PROBE_TIMEOUT_MS: u64 = 2_000;
const FUNCTIONAL_OUTPUT_PROBE_FRAMES: usize = 128;
const MAX_DELIVERY_DEADLINE_MS: u64 = 120_000;
const MIN_DELIVERY_DEADLINE_MS: u64 = 10_000;

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn projected_unix_ms(anchor: Instant, anchor_unix_ms: u128, target: Instant) -> u128 {
    target
        .checked_duration_since(anchor)
        .map(|delta| anchor_unix_ms.saturating_add(delta.as_millis()))
        .unwrap_or(anchor_unix_ms)
}

#[derive(Debug, Clone)]
pub struct MeetingOutputDeliveryReport {
    pub ok: bool,
    pub execution_attempted: bool,
    pub first_playback_at: Option<Instant>,
    pub first_playback_unix_ms: Option<u128>,
    pub blocker: String,
}

#[derive(Clone)]
struct MeetingOutputCancelControl {
    generation: u64,
    cancel_requested: Arc<AtomicBool>,
}

#[derive(Clone)]
struct PreparedMeetingOutputDevice {
    name: String,
    device: cpal::Device,
}

static MEETING_OUTPUT_CANCEL_CONTROL: OnceLock<Mutex<Option<MeetingOutputCancelControl>>> =
    OnceLock::new();
static PREPARED_MEETING_OUTPUT_DEVICE: OnceLock<Mutex<Option<PreparedMeetingOutputDevice>>> =
    OnceLock::new();

fn cancel_control() -> &'static Mutex<Option<MeetingOutputCancelControl>> {
    MEETING_OUTPUT_CANCEL_CONTROL.get_or_init(|| Mutex::new(None))
}

fn prepared_output_device_store() -> &'static Mutex<Option<PreparedMeetingOutputDevice>> {
    PREPARED_MEETING_OUTPUT_DEVICE.get_or_init(|| Mutex::new(None))
}

fn find_output_device(requested_name: &str) -> Result<cpal::Device, String> {
    let requested_name = requested_name.trim();
    if requested_name.is_empty() {
        return Err("meeting_output:selected_output_device_missing".to_string());
    }

    let host = cpal::default_host();
    let devices = host
        .output_devices()
        .map_err(|_| "meeting_output:output_device_enumeration_failed".to_string())?;
    for device in devices {
        if device.name().ok().as_deref() == Some(requested_name) {
            return Ok(device);
        }
    }
    Err("meeting_output:selected_output_device_unavailable".to_string())
}

pub fn prepare_meeting_output_device(requested_name: &str) -> Result<(), String> {
    let requested_name = requested_name.trim();
    let device = find_output_device(requested_name)?;
    let config = device
        .default_output_config()
        .map_err(|_| "meeting_output:default_output_config_unavailable".to_string())?;
    let channels = config.channels();
    let sample_rate = config.sample_rate().0;
    if channels == 0 || channels > MAX_OUTPUT_CHANNELS {
        return Err("meeting_output:unsupported_output_channel_count".to_string());
    }
    if !(MIN_OUTPUT_SAMPLE_RATE_HZ..=MAX_OUTPUT_SAMPLE_RATE_HZ).contains(&sample_rate) {
        return Err("meeting_output:unsupported_output_sample_rate".to_string());
    }
    match config.sample_format() {
        cpal::SampleFormat::F32 | cpal::SampleFormat::I16 | cpal::SampleFormat::U16 => {}
        other => {
            return Err(format!(
                "meeting_output:unsupported_output_sample_format:{other:?}"
            ))
        }
    }

    let mut prepared = prepared_output_device_store()
        .lock()
        .map_err(|_| "meeting_output:prepared_device_state_lock_failed".to_string())?;
    *prepared = Some(PreparedMeetingOutputDevice {
        name: requested_name.to_string(),
        device,
    });
    Ok(())
}

fn prepared_output_device(requested_name: &str) -> Result<cpal::Device, String> {
    let requested_name = requested_name.trim();
    let prepared = prepared_output_device_store()
        .lock()
        .map_err(|_| "meeting_output:prepared_device_state_lock_failed".to_string())?;
    let Some(prepared) = prepared.as_ref() else {
        return Err("meeting_output:prepared_output_device_missing".to_string());
    };
    if prepared.name != requested_name {
        return Err("meeting_output:prepared_output_device_mismatch".to_string());
    }
    Ok(prepared.device.clone())
}

pub fn clear_prepared_meeting_output_device() {
    if let Ok(mut prepared) = prepared_output_device_store().lock() {
        *prepared = None;
    }
}

struct PlaybackCursor {
    samples: Arc<Vec<f32>>,
    index: usize,
    completed: bool,
    first_playback_signalled: bool,
    completion_tx: mpsc::SyncSender<Duration>,
    first_playback_tx: mpsc::SyncSender<Instant>,
    cancel_requested: Arc<AtomicBool>,
    generation: u64,
    sample_rate_hz: u32,
    channels: u16,
}

impl PlaybackCursor {
    fn fill<T: Copy>(
        &mut self,
        data: &mut [T],
        callback_info: &cpal::OutputCallbackInfo,
        convert: impl Fn(f32) -> T,
    ) {
        let cancelled = self.cancel_requested.load(Ordering::Acquire)
            || !runtime_generation_is_authoritative(self.generation);
        if cancelled {
            self.cancel_requested.store(true, Ordering::Release);
            data.fill(convert(0.0));
            self.signal_complete(Duration::ZERO);
            return;
        }

        for target in data.iter_mut() {
            if let Some(value) = self.samples.get(self.index).copied() {
                if !self.first_playback_signalled {
                    self.signal_first_playback(callback_info);
                }
                *target = convert(value);
                self.index += 1;
            } else {
                *target = convert(0.0);
            }
        }

        if self.index >= self.samples.len() {
            let frames = data.len() / usize::from(self.channels.max(1));
            let tail = if self.sample_rate_hz == 0 {
                Duration::ZERO
            } else {
                Duration::from_secs_f64(frames as f64 / self.sample_rate_hz as f64)
            };
            self.signal_complete(tail);
        }
    }

    fn signal_first_playback(&mut self, callback_info: &cpal::OutputCallbackInfo) {
        if self.first_playback_signalled {
            return;
        }
        self.first_playback_signalled = true;
        let timestamp = callback_info.timestamp();
        let playback_delay = timestamp
            .playback
            .duration_since(&timestamp.callback)
            .unwrap_or(Duration::ZERO);
        let callback_at = Instant::now();
        let predicted_playback_at = callback_at
            .checked_add(playback_delay)
            .unwrap_or(callback_at);
        let _ = self.first_playback_tx.try_send(predicted_playback_at);
    }

    fn signal_complete(&mut self, tail: Duration) {
        if !self.completed {
            self.completed = true;
            let _ = self.completion_tx.try_send(tail);
        }
    }
}

fn build_output_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    samples: Arc<Vec<f32>>,
    cancel_requested: Arc<AtomicBool>,
    generation: u64,
    completion_tx: mpsc::SyncSender<Duration>,
    first_playback_tx: mpsc::SyncSender<Instant>,
    callback_errors: Arc<Mutex<Vec<String>>>,
) -> Result<cpal::Stream, String> {
    let make_cursor = |tx: mpsc::SyncSender<Duration>| PlaybackCursor {
        samples: Arc::clone(&samples),
        index: 0,
        completed: false,
        first_playback_signalled: false,
        completion_tx: tx,
        first_playback_tx: first_playback_tx.clone(),
        cancel_requested: Arc::clone(&cancel_requested),
        generation,
        sample_rate_hz: config.sample_rate.0,
        channels: config.channels,
    };

    match sample_format {
        cpal::SampleFormat::F32 => {
            let mut cursor = make_cursor(completion_tx.clone());
            let errors = Arc::clone(&callback_errors);
            let error_tx = completion_tx.clone();
            device
                .build_output_stream(
                    config,
                    move |data: &mut [f32], info| cursor.fill(data, info, |value| value),
                    move |error| {
                        if let Ok(mut stored) = errors.lock() {
                            stored.push(error.to_string());
                        }
                        let _ = error_tx.try_send(Duration::ZERO);
                    },
                    None,
                )
                .map_err(|_| "meeting_output:stream_build_failed".to_string())
        }
        cpal::SampleFormat::I16 => {
            let mut cursor = make_cursor(completion_tx.clone());
            let errors = Arc::clone(&callback_errors);
            let error_tx = completion_tx.clone();
            device
                .build_output_stream(
                    config,
                    move |data: &mut [i16], info| {
                        cursor.fill(data, info, |value| {
                            (value.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
                        })
                    },
                    move |error| {
                        if let Ok(mut stored) = errors.lock() {
                            stored.push(error.to_string());
                        }
                        let _ = error_tx.try_send(Duration::ZERO);
                    },
                    None,
                )
                .map_err(|_| "meeting_output:stream_build_failed".to_string())
        }
        cpal::SampleFormat::U16 => {
            let mut cursor = make_cursor(completion_tx.clone());
            let errors = Arc::clone(&callback_errors);
            let error_tx = completion_tx.clone();
            device
                .build_output_stream(
                    config,
                    move |data: &mut [u16], info| {
                        cursor.fill(data, info, |value| {
                            (((value.clamp(-1.0, 1.0) + 1.0) * 0.5) * u16::MAX as f32).round()
                                as u16
                        })
                    },
                    move |error| {
                        if let Ok(mut stored) = errors.lock() {
                            stored.push(error.to_string());
                        }
                        let _ = error_tx.try_send(Duration::ZERO);
                    },
                    None,
                )
                .map_err(|_| "meeting_output:stream_build_failed".to_string())
        }
        _ => Err("meeting_output:unsupported_output_sample_format".to_string()),
    }
}

pub fn probe_prepared_meeting_output_device_functionally(
    requested_name: &str,
    generation: u64,
) -> Result<(), String> {
    if !runtime_generation_is_authoritative(generation) {
        return Err("meeting_output:functional_probe_generation_not_authoritative".to_string());
    }

    let device = prepared_output_device(requested_name)?;
    let supported = device
        .default_output_config()
        .map_err(|_| "meeting_output:functional_probe_default_config_unavailable".to_string())?;
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    if config.channels == 0 || config.channels > MAX_OUTPUT_CHANNELS {
        return Err("meeting_output:functional_probe_unsupported_channel_count".to_string());
    }
    if !(MIN_OUTPUT_SAMPLE_RATE_HZ..=MAX_OUTPUT_SAMPLE_RATE_HZ).contains(&config.sample_rate.0) {
        return Err("meeting_output:functional_probe_unsupported_sample_rate".to_string());
    }

    // The probe must prove the native endpoint can actually build, start, and invoke
    // its callback without emitting speech. Zero-valued frames are sufficient: C5
    // verifies endpoint execution, not meeting-app reception or audible content.
    let sample_count = usize::from(config.channels).saturating_mul(FUNCTIONAL_OUTPUT_PROBE_FRAMES);
    let samples = Arc::new(vec![0.0_f32; sample_count]);
    let cancel_requested = Arc::new(AtomicBool::new(false));
    let (completion_tx, _completion_rx) = mpsc::sync_channel(1);
    let (first_playback_tx, first_playback_rx) = mpsc::sync_channel(1);
    let callback_errors = Arc::new(Mutex::new(Vec::new()));
    let stream = build_output_stream(
        &device,
        &config,
        sample_format,
        samples,
        cancel_requested,
        generation,
        completion_tx,
        first_playback_tx,
        Arc::clone(&callback_errors),
    )?;
    stream
        .play()
        .map_err(|_| "meeting_output:functional_probe_stream_start_failed".to_string())?;

    let callback =
        first_playback_rx.recv_timeout(Duration::from_millis(FUNCTIONAL_OUTPUT_PROBE_TIMEOUT_MS));
    let callback_error = callback_errors
        .lock()
        .ok()
        .and_then(|errors| errors.last().cloned());
    let still_authoritative = runtime_generation_is_authoritative(generation);
    drop(stream);

    if !still_authoritative {
        return Err("meeting_output:functional_probe_generation_revoked".to_string());
    }
    if callback_error.is_some() {
        return Err("meeting_output:functional_probe_callback_failed".to_string());
    }
    match callback {
        Ok(_) => Ok(()),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err("meeting_output:functional_probe_callback_timeout".to_string())
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("meeting_output:functional_probe_callback_disconnected".to_string())
        }
    }
}

fn install_cancel_control(generation: u64) -> Result<Arc<AtomicBool>, String> {
    let mut guard = cancel_control()
        .lock()
        .map_err(|_| "meeting_output:cancel_state_lock_failed".to_string())?;
    if let Some(existing) = guard.as_ref() {
        if !existing.cancel_requested.load(Ordering::Acquire) {
            return Err("meeting_output:delivery_already_active".to_string());
        }
    }
    let cancel_requested = Arc::new(AtomicBool::new(false));
    *guard = Some(MeetingOutputCancelControl {
        generation,
        cancel_requested: Arc::clone(&cancel_requested),
    });
    Ok(cancel_requested)
}

fn clear_cancel_control(generation: u64) {
    if let Ok(mut guard) = cancel_control().lock() {
        if guard.as_ref().map(|value| value.generation) == Some(generation) {
            *guard = None;
        }
    }
}

pub fn cancel_meeting_output_for_generation(generation: u64) -> bool {
    let Ok(guard) = cancel_control().lock() else {
        return false;
    };
    let Some(control) = guard.as_ref() else {
        return true;
    };
    if control.generation != generation {
        return true;
    }
    control.cancel_requested.store(true, Ordering::Release);
    true
}

fn blocked(blocker: &str, execution_attempted: bool) -> MeetingOutputDeliveryReport {
    MeetingOutputDeliveryReport {
        ok: false,
        execution_attempted,
        first_playback_at: None,
        first_playback_unix_ms: None,
        blocker: blocker.to_string(),
    }
}

pub fn deliver_meeting_output_wav(
    source_audio_path: &str,
    selected_output_device: Option<&str>,
    generation: u64,
) -> MeetingOutputDeliveryReport {
    if !runtime_generation_is_authoritative(generation) {
        return blocked("meeting_output:generation_not_authoritative", false);
    }
    let Some(output_name) = selected_output_device
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return blocked("meeting_output:selected_output_device_missing", false);
    };

    let wav = match decode_wav_file(Path::new(source_audio_path)) {
        Ok(value) => value,
        Err(blocker) => {
            return blocked(&blocker, false)
        }
    };
    let device = match prepared_output_device(output_name) {
        Ok(value) => value,
        Err(blocker) => {
            return blocked(&blocker, false)
        }
    };
    let supported = match device.default_output_config() {
        Ok(value) => value,
        Err(_) => {
            return blocked("meeting_output:default_output_config_unavailable", false)
        }
    };
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let samples = Arc::new(prepare_output_samples(
        &wav,
        config.sample_rate.0,
        config.channels,
    ));
    if samples.is_empty() {
        return blocked("meeting_output:prepared_audio_empty", false);
    }

    let delivery_started_at = Instant::now();
    let delivery_started_unix_ms = current_unix_ms();
    let cancel_requested = match install_cancel_control(generation) {
        Ok(value) => value,
        Err(blocker) => {
            return blocked(&blocker, false)
        }
    };
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    let (first_playback_tx, first_playback_rx) = mpsc::sync_channel(1);
    let callback_errors = Arc::new(Mutex::new(Vec::new()));
    let stream = match build_output_stream(
        &device,
        &config,
        sample_format,
        Arc::clone(&samples),
        Arc::clone(&cancel_requested),
        generation,
        completion_tx,
        first_playback_tx,
        Arc::clone(&callback_errors),
    ) {
        Ok(value) => value,
        Err(blocker) => {
            clear_cancel_control(generation);
            return blocked(&blocker, false);
        }
    };
    if stream.play().is_err() {
        clear_cancel_control(generation);
        return blocked("meeting_output:stream_start_failed", true);
    }

    let deadline = Duration::from_millis(delivery_deadline_ms(
        samples.len(),
        config.channels,
        config.sample_rate.0,
    ));
    let completion = completion_rx.recv_timeout(deadline);
    let callback_error = callback_errors
        .lock()
        .ok()
        .and_then(|errors| errors.last().cloned());
    let first_playback_at = first_playback_rx.try_recv().ok();
    let first_playback_unix_ms = first_playback_at
        .map(|instant| projected_unix_ms(delivery_started_at, delivery_started_unix_ms, instant));
    let cancelled = cancel_requested.load(Ordering::Acquire)
        || !runtime_generation_is_authoritative(generation);

    match completion {
        Ok(tail) if !cancelled && callback_error.is_none() => {
            if !tail.is_zero() {
                thread::sleep(tail.min(Duration::from_secs(1)));
            }
        }
        Ok(_) => {}
        Err(_) => {
            cancel_requested.store(true, Ordering::Release);
        }
    }
    drop(stream);
    clear_cancel_control(generation);

    if cancelled || cancel_requested.load(Ordering::Acquire) && completion.is_ok() {
        let mut report = blocked("meeting_output:cancelled", true);
        report.first_playback_at = first_playback_at;
        report.first_playback_unix_ms = first_playback_unix_ms;
        return report;
    }
    if callback_error.is_some() {
        let mut report = blocked("meeting_output:stream_callback_failed", true);
        report.first_playback_at = first_playback_at;
        report.first_playback_unix_ms = first_playback_unix_ms;
        return report;
    }
    if completion.is_err() {
        let mut report = blocked("meeting_output:delivery_deadline_exceeded", true);
        report.first_playback_at = first_playback_at;
        report.first_playback_unix_ms = first_playback_unix_ms;
        return report;
    }

    MeetingOutputDeliveryReport {
        ok: true,
        execution_attempted: true,
        first_playback_at,
        first_playback_unix_ms,
        blocker: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decode_wav_bytes, delivery_deadline_ms, prepare_output_samples, projected_unix_ms,
    };
    use std::time::{Duration, Instant};

    fn pcm16_wav(sample_rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data_bytes = (samples.len() * 2) as u32;
        let block_align = channels * 2;
        let byte_rate = sample_rate * u32::from(block_align);
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_bytes.to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        wav
    }

    #[test]
    fn decodes_pcm16_and_resamples_to_output_channels() {
        let wav = pcm16_wav(22_050, 1, &[0, 8_000, -8_000, 0]);
        let decoded = decode_wav_bytes(&wav).expect("PCM16 fixture should decode");
        assert_eq!(decoded.sample_rate_hz, 22_050);
        assert_eq!(decoded.channels, 1);
        assert_eq!(decoded.samples.len(), 4);

        let prepared = prepare_output_samples(&decoded, 44_100, 2);
        assert_eq!(prepared.len(), 16);
        assert!(prepared.iter().all(|sample| (-1.0..=1.0).contains(sample)));
    }

    #[test]
    fn delivery_deadline_is_bounded_and_duration_derived() {
        assert_eq!(delivery_deadline_ms(48_000 * 2, 2, 48_000), 10_000);
        assert_eq!(delivery_deadline_ms(48_000 * 2 * 80, 2, 48_000), 120_000);
    }

    #[test]
    fn rejects_non_wav_bytes() {
        assert!(decode_wav_bytes(b"not a wav").is_err());
    }

    #[test]
    fn predicted_playback_unix_time_uses_monotonic_delta() {
        let anchor = Instant::now();
        let predicted = anchor + Duration::from_millis(37);
        assert_eq!(projected_unix_ms(anchor, 10_000, predicted), 10_037);
    }
}
