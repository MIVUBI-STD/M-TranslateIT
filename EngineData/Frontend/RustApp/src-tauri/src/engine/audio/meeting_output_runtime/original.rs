//! Original microphone playback through the same prepared VB-CABLE render endpoint.
//! The original stream is only alive while the user selects Original; the
//! translated WAV output and its existing CPAL delivery remain the other source.

use cpal::traits::{DeviceTrait, StreamTrait};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::engine::audio::original_voice_pcm;
use crate::engine::runtime_state::runtime_generation_is_authoritative;

use super::{prepared_output_device, MAX_OUTPUT_CHANNELS, MAX_OUTPUT_SAMPLE_RATE_HZ, MIN_OUTPUT_SAMPLE_RATE_HZ};

const START_TIMEOUT: Duration = Duration::from_secs(3);

struct OriginalOutput {
    generation: u64,
    stop_tx: SyncSender<()>,
    thread: JoinHandle<()>,
}

static OUTPUT: OnceLock<Mutex<Option<OriginalOutput>>> = OnceLock::new();

fn output() -> &'static Mutex<Option<OriginalOutput>> {
    OUTPUT.get_or_init(|| Mutex::new(None))
}

fn run(
    device_name: &str,
    generation: u64,
    source_epoch: u64,
    stop_rx: mpsc::Receiver<()>,
    ready_tx: SyncSender<Result<(), String>>,
) {
    let result = (|| -> Result<cpal::Stream, String> {
        let device = prepared_output_device(device_name)?;
        let supported = device.default_output_config()
            .map_err(|_| "original_voice:output_config_unavailable".to_string())?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        if config.channels == 0 || config.channels > MAX_OUTPUT_CHANNELS
            || !(MIN_OUTPUT_SAMPLE_RATE_HZ..=MAX_OUTPUT_SAMPLE_RATE_HZ).contains(&config.sample_rate.0)
        {
            return Err("original_voice:unsupported_output_configuration".into());
        }

        let channels = config.channels;
        let rate = config.sample_rate.0;
        let error_cb = |_error: cpal::StreamError| {
            // The source authority remains closed on callback failures until Stop;
            // normal device-loss diagnostics own user-facing error classification.
        };
        let stream = match format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config,
                move |samples: &mut [f32], _| {
                    original_voice_pcm::try_render_native(
                        samples, rate, channels, generation, source_epoch,
                        0.0_f32, |sample| sample,
                    );
                },
                error_cb,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                &config,
                move |samples: &mut [i16], _| {
                    original_voice_pcm::try_render_native(
                        samples, rate, channels, generation, source_epoch,
                        0_i16, |sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16,
                    );
                },
                error_cb,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_output_stream(
                &config,
                move |samples: &mut [u16], _| {
                    original_voice_pcm::try_render_native(
                        samples, rate, channels, generation, source_epoch,
                        u16::MAX / 2 + 1,
                        |sample| (((sample.clamp(-1.0, 1.0) + 1.0) * 0.5) * u16::MAX as f32).round() as u16,
                    );
                },
                error_cb,
                None,
            ),
            _ => return Err("original_voice:unsupported_output_sample_format".into()),
        }.map_err(|_| "original_voice:stream_build_failed".to_string())?;
        stream.play().map_err(|_| "original_voice:stream_start_failed".to_string())?;
        Ok(stream)
    })();

    match result {
        Ok(stream) => {
            let _ = ready_tx.send(Ok(()));
            let _ = stop_rx.recv();
            drop(stream);
        }
        Err(error) => { let _ = ready_tx.send(Err(error)); }
    }
}

/// Source owner calls this after switching away from translated WAV delivery.
/// The same previously prepared render endpoint is used; there is no fallback.
pub fn start_original_voice_output(device_name: &str, generation: u64, source_epoch: u64) -> Result<(), String> {
    let mut guard = output().lock().map_err(|_| "original_voice:output_lock_failed".to_string())?;
    if guard.is_some() { return Err("original_voice:output_already_active".into()); }
    if !runtime_generation_is_authoritative(generation) {
        return Err("original_voice:generation_not_authoritative".into());
    }
    // Preparation happens before enabling callback capture; a failed start
    // returns to the prior translated mode without leaking original samples.
    let _ = prepared_output_device(device_name)?;
    original_voice_pcm::begin(generation, source_epoch).map_err(str::to_string)?;

    let (stop_tx, stop_rx) = mpsc::sync_channel(1);
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let name = device_name.to_string();
    let thread = match thread::Builder::new().name("translateit-original-output".into())
        .spawn(move || run(&name, generation, source_epoch, stop_rx, ready_tx))
    {
        Ok(value) => value,
        Err(_) => {
            original_voice_pcm::stop(generation);
            return Err("original_voice:output_thread_failed".into());
        }
    };
    let result = ready_rx.recv_timeout(START_TIMEOUT);
    if !matches!(result, Ok(Ok(()))) || !runtime_generation_is_authoritative(generation) {
        original_voice_pcm::stop(generation);
        let _ = stop_tx.try_send(());
        let _ = thread.join();
        return match result {
            Ok(Err(error)) => Err(error),
            _ => Err("original_voice:output_start_unavailable".into()),
        };
    }
    *guard = Some(OriginalOutput { generation, stop_tx, thread });
    Ok(())
}

/// Stop is idempotent and does not affect an output belonging to another session.
pub fn stop_original_voice_output(generation: u64) -> bool {
    let mut guard = match output().lock() { Ok(value) => value, Err(_) => return false };
    if guard.as_ref().is_some_and(|runtime| runtime.generation != generation) {
        return false;
    }
    original_voice_pcm::stop(generation);
    let Some(runtime) = guard.take() else { return true };
    let _ = runtime.stop_tx.try_send(());
    runtime.thread.join().is_ok()
}
