use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use super::device_selection::resolve_unique_named_device;
use super::{TARGET_CHANNELS, TARGET_SAMPLE_RATE_HZ};

const MAX_INPUT_DEVICE_NAME_CHARS: usize = 160;
const FUNCTIONAL_INPUT_PROBE_TIMEOUT_MS: u64 = 2_000;

#[derive(Debug, Clone, Serialize)]
pub struct InputPreparationStatus {
    pub backend_id: String,
    pub input_device_name: Option<String>,
    pub target_sample_rate_hz: u32,
    pub target_channels: u16,
    pub prepared: bool,
    pub running: bool,
    pub functional_verified: bool,
    pub callback_frames_observed: u64,
    pub blocker: String,
    pub note: String,
}

impl InputPreparationStatus {
    pub fn inspect_input_device(requested_name: Option<&str>) -> Self {
        let host = cpal::default_host();
        let backend_id = format!("cpal-{}", host.id().name());
        let requested_name = requested_name
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let device = match resolve_input_device(&host, requested_name) {
            Ok((device, _)) => device,
            Err(error) => {
                return Self::blocked(
                    backend_id,
                    requested_name.map(str::to_string),
                    "audio_input:device_unavailable",
                    &error,
                )
            }
        };

        let device_name = safe_device_name(&device);
        let Ok(config) = device.default_input_config() else {
            return Self::blocked(
                backend_id,
                device_name,
                "audio_input:no_input_config",
                "The selected microphone exists, but no default input configuration was available.",
            );
        };

        // Device acceptance is functional: the capture path converts/resamples the
        // native configuration at the segment boundary, so native rate/channels are
        // reported for honesty but never hard-gate device acceptance here.
        let native_sample_rate_hz = config.sample_rate().0;
        let native_channels = config.channels();

        Self {
            backend_id,
            input_device_name: device_name,
            target_sample_rate_hz: TARGET_SAMPLE_RATE_HZ,
            target_channels: TARGET_CHANNELS,
            prepared: true,
            running: false,
            functional_verified: false,
            callback_frames_observed: 0,
            blocker: String::new(),
            note: format!(
                "{} available with a usable default input configuration (native {} Hz x {} channel(s)); the pipeline converts/resamples at segment boundary. This routine status check does not open a capture stream.",
                if requested_name.is_some() {
                    "The selected microphone is"
                } else {
                    "The Windows default microphone is"
                },
                native_sample_rate_hz,
                native_channels
            ),
        }
    }

    fn blocked(
        backend_id: String,
        input_device_name: Option<String>,
        blocker: &str,
        note: &str,
    ) -> Self {
        Self {
            backend_id,
            input_device_name,
            target_sample_rate_hz: TARGET_SAMPLE_RATE_HZ,
            target_channels: TARGET_CHANNELS,
            prepared: false,
            running: false,
            functional_verified: false,
            callback_frames_observed: 0,
            blocker: blocker.to_string(),
            note: note.to_string(),
        }
    }
}

pub fn probe_input_device_functionally(requested_name: Option<&str>) -> InputPreparationStatus {
    let started = Instant::now();
    let host = cpal::default_host();
    let backend_id = format!("cpal-{}", host.id().name());
    let requested_name = requested_name
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let device = match resolve_input_device(&host, requested_name) {
        Ok((device, _)) => device,
        Err(error) => {
            return InputPreparationStatus::blocked(
                backend_id,
                requested_name.map(str::to_string),
                "audio_input:functional_probe_device_unavailable",
                &error,
            )
        }
    };
    let device_name = safe_device_name(&device);
    let supported = match device.default_input_config() {
        Ok(config) => config,
        Err(error) => {
            return InputPreparationStatus::blocked(
                backend_id,
                device_name,
                "audio_input:functional_probe_no_input_config",
                &format!(
                    "The selected microphone could not provide a default input configuration for functional verification: {error}"
                ),
            )
        }
    };

    let sample_format = supported.sample_format();
    let stream_config = supported.config();
    let callback_frames = Arc::new(AtomicU64::new(0));
    let (event_tx, event_rx) = mpsc::sync_channel::<Result<(), String>>(1);
    let stream = match build_functional_probe_stream(
        &device,
        &stream_config,
        sample_format,
        Arc::clone(&callback_frames),
        event_tx,
    ) {
        Ok(stream) => stream,
        Err(error) => {
            return InputPreparationStatus::blocked(
                backend_id,
                device_name,
                "audio_input:functional_probe_stream_build_failed",
                &format!("The selected microphone could not open a verification stream: {error}"),
            )
        }
    };

    if let Err(error) = stream.play() {
        drop(stream);
        return InputPreparationStatus::blocked(
            backend_id,
            device_name,
            "audio_input:functional_probe_stream_start_failed",
            &format!("The selected microphone verification stream could not start: {error}"),
        );
    }

    let event = event_rx.recv_timeout(Duration::from_millis(FUNCTIONAL_INPUT_PROBE_TIMEOUT_MS));
    let observed = callback_frames.load(Ordering::SeqCst);
    drop(stream);

    match event {
        Ok(Ok(())) if observed > 0 => InputPreparationStatus {
            backend_id,
            input_device_name: device_name,
            target_sample_rate_hz: TARGET_SAMPLE_RATE_HZ,
            target_channels: TARGET_CHANNELS,
            prepared: true,
            running: false,
            functional_verified: true,
            callback_frames_observed: observed,
            blocker: String::new(),
            note: format!(
                "The microphone opened successfully and delivered native callback frames within {} ms. No microphone samples were retained by this verification.",
                started.elapsed().as_millis()
            ),
        },
        Ok(Ok(())) => InputPreparationStatus::blocked(
            backend_id,
            device_name,
            "audio_input:functional_probe_no_frames",
            "The microphone callback opened but did not deliver any audio frames during functional verification.",
        ),
        Ok(Err(error)) => InputPreparationStatus::blocked(
            backend_id,
            device_name,
            "audio_input:functional_probe_callback_failed",
            &format!("The microphone verification callback failed: {error}"),
        ),
        Err(mpsc::RecvTimeoutError::Timeout) => InputPreparationStatus::blocked(
            backend_id,
            device_name,
            "audio_input:functional_probe_timeout",
            "The microphone stream opened, but no audio callback frames arrived before the bounded verification timeout.",
        ),
        Err(mpsc::RecvTimeoutError::Disconnected) => InputPreparationStatus::blocked(
            backend_id,
            device_name,
            "audio_input:functional_probe_channel_closed",
            "The microphone verification stream ended before callback readiness could be confirmed.",
        ),
    }
}

fn build_functional_probe_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    callback_frames: Arc<AtomicU64>,
    event_tx: mpsc::SyncSender<Result<(), String>>,
) -> Result<cpal::Stream, String> {
    let channels = usize::from(config.channels.max(1));

    macro_rules! build_probe_stream {
        ($sample:ty) => {{
            let frames = Arc::clone(&callback_frames);
            let ready_tx = event_tx.clone();
            let error_tx = event_tx.clone();
            device
                .build_input_stream(
                    config,
                    move |data: &[$sample], _| {
                        let frame_count = data.len() / channels;
                        if frame_count > 0 {
                            frames.fetch_add(frame_count as u64, Ordering::SeqCst);
                            let _ = ready_tx.try_send(Ok(()));
                        }
                    },
                    move |error| {
                        let _ = error_tx.try_send(Err(error.to_string()));
                    },
                    None,
                )
                .map_err(|error| error.to_string())
        }};
    }

    match sample_format {
        cpal::SampleFormat::F32 => build_probe_stream!(f32),
        cpal::SampleFormat::I16 => build_probe_stream!(i16),
        cpal::SampleFormat::U16 => build_probe_stream!(u16),
        other => Err(format!(
            "Unsupported default microphone sample format for the active capture path: {other:?}"
        )),
    }
}

fn resolve_input_device(
    host: &cpal::Host,
    requested_name: Option<&str>,
) -> Result<(cpal::Device, bool), String> {
    if let Some(requested_name) = requested_name {
        return match find_named_input_device(host, requested_name) {
            Ok(Some(device)) => Ok((device, false)),
            Ok(None) => Err(
                "The selected microphone is not available. The previous/default microphone was not substituted."
                    .to_string(),
            ),
            Err(error) => Err(format!(
                "The selected microphone could not be enumerated: {error}"
            )),
        };
    }

    host.default_input_device()
        .map(|device| (device, true))
        .ok_or_else(|| "No Windows default input device was found.".to_string())
}

fn find_named_input_device(
    host: &cpal::Host,
    requested_name: &str,
) -> Result<Option<cpal::Device>, String> {
    let devices = host.input_devices().map_err(|error| error.to_string())?;
    resolve_unique_named_device(devices, requested_name, safe_device_name)
        .map_err(|_| "Several microphones have the selected name; no microphone was substituted.".to_string())
}

fn is_unsafe_input_name_character(character: char) -> bool {
    character == '\0'
        || ('\u{0001}'..='\u{0008}').contains(&character)
        || ('\u{000b}'..='\u{001f}').contains(&character)
        || character == '\u{007f}'
        || ('\u{202a}'..='\u{202e}').contains(&character)
        || ('\u{2066}'..='\u{2069}').contains(&character)
}

fn safe_device_name(device: &cpal::Device) -> Option<String> {
    let clean = device
        .name()
        .ok()?
        .trim()
        .chars()
        .filter(|character| !is_unsafe_input_name_character(*character))
        .take(MAX_INPUT_DEVICE_NAME_CHARS)
        .collect::<String>()
        .trim()
        .to_string();
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}
