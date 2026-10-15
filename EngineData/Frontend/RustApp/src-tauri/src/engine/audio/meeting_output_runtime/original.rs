//! Original microphone playback through the same prepared VB-CABLE render endpoint.
//! The original stream is only alive while the user selects Original; the
//! translated WAV output and its existing CPAL delivery remain the other source.

use cpal::traits::{DeviceTrait, StreamTrait};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
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
    callback_failed: Arc<AtomicBool>,
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
    callback_failed: Arc<AtomicBool>,
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
        let (callback_tx, callback_rx) = mpsc::sync_channel::<Result<(), String>>(1);
        let failure_tx = callback_tx.clone();
        let failure_state = Arc::clone(&callback_failed);
        let error_cb = move |_error: cpal::StreamError| {
            failure_state.store(true, Ordering::Release);
            let _ = failure_tx.try_send(Err("original_voice:stream_callback_failed".to_string()));
        };
        let stream = match format {
            cpal::SampleFormat::F32 => {
                let output_failed = Arc::clone(&callback_failed);
                let confirmation = callback_tx.clone();
                let mut first_callback = true;
                device.build_output_stream(
                &config,
                move |samples: &mut [f32], _| {
                    if first_callback && !samples.is_empty() {
                        first_callback = false;
                        let _ = confirmation.try_send(Ok(()));
                    }
                    if output_failed.load(Ordering::Acquire) {
                        samples.fill(0.0_f32);
                        return;
                    }
                    original_voice_pcm::try_render_native(
                        samples, rate, channels, generation, source_epoch,
                        0.0_f32, |sample| sample,
                    );
                },
                error_cb,
                None,
            )
            },
            cpal::SampleFormat::I16 => {
                let output_failed = Arc::clone(&callback_failed);
                let confirmation = callback_tx.clone();
                let mut first_callback = true;
                device.build_output_stream(
                &config,
                move |samples: &mut [i16], _| {
                    if first_callback && !samples.is_empty() {
                        first_callback = false;
                        let _ = confirmation.try_send(Ok(()));
                    }
                    if output_failed.load(Ordering::Acquire) {
                        samples.fill(0_i16);
                        return;
                    }
                    original_voice_pcm::try_render_native(
                        samples, rate, channels, generation, source_epoch,
                        0_i16, |sample| (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16,
                    );
                },
                error_cb,
                None,
            )
            },
            cpal::SampleFormat::U16 => {
                let output_failed = Arc::clone(&callback_failed);
                let confirmation = callback_tx.clone();
                let mut first_callback = true;
                device.build_output_stream(
                &config,
                move |samples: &mut [u16], _| {
                    if first_callback && !samples.is_empty() {
                        first_callback = false;
                        let _ = confirmation.try_send(Ok(()));
                    }
                    if output_failed.load(Ordering::Acquire) {
                        samples.fill(u16::MAX / 2 + 1);
                        return;
                    }
                    original_voice_pcm::try_render_native(
                        samples, rate, channels, generation, source_epoch,
                        u16::MAX / 2 + 1,
                        |sample| (((sample.clamp(-1.0, 1.0) + 1.0) * 0.5) * u16::MAX as f32).round() as u16,
                    );
                },
                error_cb,
                None,
            )
            },
            _ => return Err("original_voice:unsupported_output_sample_format".into()),
        }.map_err(|_| "original_voice:stream_build_failed".to_string())?;
        stream.play().map_err(|_| "original_voice:stream_start_failed".to_string())?;
        match callback_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(Ok(())) => Ok(stream),
            Ok(Err(blocker)) => Err(blocker),
            Err(_) => Err("original_voice:stream_callback_timeout".to_string()),
        }
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
    let callback_failed = Arc::new(AtomicBool::new(false));
    let thread_failed = Arc::clone(&callback_failed);
    let thread = match thread::Builder::new().name("translateit-original-output".into())
        .spawn(move || run(&name, generation, source_epoch, stop_rx, ready_tx, thread_failed))
    {
        Ok(value) => value,
        Err(_) => {
            original_voice_pcm::stop(generation);
            return Err("original_voice:output_thread_failed".into());
        }
    };
    let result = ready_rx.recv_timeout(START_TIMEOUT);
    if !matches!(&result, Ok(Ok(()))) || !runtime_generation_is_authoritative(generation) {
        original_voice_pcm::stop(generation);
        let _ = stop_tx.try_send(());
        // A driver may stall in its native stream start. Do not join a thread
        // that failed our deadline; retain its owner for bounded Stop retry.
        if !thread.is_finished() {
            *guard = Some(OriginalOutput { generation, stop_tx, thread, callback_failed: Arc::clone(&callback_failed) });
            return Err("original_voice:output_start_unconfirmed_cleanup_required".into());
        }
        let _ = thread.join();
        return match result {
            Ok(Err(error)) => Err(error),
            _ => Err("original_voice:output_start_unavailable".into()),
        };
    }
    *guard = Some(OriginalOutput { generation, stop_tx, thread, callback_failed: Arc::clone(&callback_failed) });
    Ok(())
}

/// True only when the current native Original stream has not reported a
/// callback fault and the owner thread has not unexpectedly terminated.
/// None means this generation has no verifiable Original output owner.
pub fn original_voice_output_healthy(generation: u64) -> Option<bool> {
    let guard = output().try_lock().ok()?;
    guard.as_ref().filter(|owner| owner.generation == generation)
        .map(|owner| !owner.callback_failed.load(Ordering::Acquire)
            && !owner.thread.is_finished())
}

/// Stop is idempotent and does not affect an output belonging to another session.
pub fn stop_original_voice_output(generation: u64) -> bool {
    let mut guard = match output().lock() { Ok(value) => value, Err(_) => return false };
    if guard.as_ref().is_some_and(|runtime| runtime.generation != generation) {
        return false;
    }
    original_voice_pcm::stop(generation);
    let Some(runtime) = guard.as_ref() else { return true };
    let _ = runtime.stop_tx.try_send(());
    for _ in 0..100 {
        if runtime.thread.is_finished() { break; }
        thread::sleep(Duration::from_millis(20));
    }
    if !runtime.thread.is_finished() {
        // Keep ownership and refuse to claim cleanup. Stop may be retried.
        return false;
    }
    guard.take().is_some_and(|runtime| runtime.thread.join().is_ok())
}
