use cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;

use crate::engine;
use crate::engine::audio::input::{probe_input_device_functionally, InputPreparationStatus};
use crate::engine::audio::live_audio_buffer::live_audio_buffer_status;

const MAX_AUDIO_DEVICE_NAME_CHARS: usize = 160;
const MAX_AUDIO_DEVICES_PER_KIND: usize = 64;

#[derive(Debug, Clone, Serialize)]
pub struct AudioQualityReport {
    pub available: bool,
    pub quality: String,
    pub label: String,
    pub rms: f32,
    pub peak: f32,
    pub clipping_ratio: f32,
    pub active_frame_ratio: f32,
    pub buffered_duration_ms: u32,
    pub blocker: String,
    pub note: String,
}

fn classify_audio_quality(
    has_audio: bool,
    buffered_duration_ms: u32,
    evidence_reason: &str,
    rms: f32,
    peak: f32,
    clipping_ratio: f32,
    active_frame_ratio: f32,
) -> AudioQualityReport {
    let unavailable = !has_audio || buffered_duration_ms < 160;
    if unavailable {
        return AudioQualityReport {
            available: false,
            quality: "unavailable".to_string(),
            label: "Run Mic Test to measure".to_string(),
            rms,
            peak,
            clipping_ratio,
            active_frame_ratio,
            buffered_duration_ms,
            blocker: "audio_quality:not_enough_live_audio".to_string(),
            note: "Audio quality uses the existing live microphone buffer. Start Mic Test or Meeting translation to measure it.".to_string(),
        };
    }

    let (quality, label, blocker, note) = if clipping_ratio > 0.025 {
        (
            "clipping",
            "Clipping",
            "audio_quality:clipping",
            "Input is clipping. Lower microphone gain or move farther from the microphone.",
        )
    } else if evidence_reason == "rejected_noise_like_impulse" {
        (
            "noisy",
            "Noisy",
            "audio_quality:noise_like_input",
            "Input contains noise-like impulses that may reduce speech recognition accuracy.",
        )
    } else if rms < 0.006 || peak < 0.021 || active_frame_ratio < 0.05 {
        (
            "too_quiet",
            "Too quiet",
            "audio_quality:too_quiet",
            "Speech level is low for the active Meeting VAD profile. Move closer or raise microphone gain.",
        )
    } else {
        (
            "good",
            "Good",
            "",
            "Input level is within the current Meeting speech thresholds and is not clipping.",
        )
    };

    AudioQualityReport {
        available: true,
        quality: quality.to_string(),
        label: label.to_string(),
        rms,
        peak,
        clipping_ratio,
        active_frame_ratio,
        buffered_duration_ms,
        blocker: blocker.to_string(),
        note: note.to_string(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceSummary {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceListReport {
    pub ok: bool,
    pub input_devices: Vec<AudioDeviceSummary>,
    pub output_devices: Vec<AudioDeviceSummary>,
    pub blocker: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioDeviceProbeReport {
    pub ok: bool,
    pub device_kind: String,
    pub requested_device_id: Option<String>,
    pub resolved_device_name: Option<String>,
    pub is_default: bool,
    pub sample_rate_hz: Option<u32>,
    pub channels: Option<u16>,
    pub blocker: String,
    pub note: String,
}

fn is_unsafe_device_name_character(character: char) -> bool {
    character == '\0'
        || ('\u{0001}'..='\u{0008}').contains(&character)
        || ('\u{000b}'..='\u{001f}').contains(&character)
        || character == '\u{007f}'
        || ('\u{202a}'..='\u{202e}').contains(&character)
        || ('\u{2066}'..='\u{2069}').contains(&character)
}

fn clean_device_name(value: String) -> Option<String> {
    let clean = value
        .trim()
        .chars()
        .filter(|character| !is_unsafe_device_name_character(*character))
        .take(MAX_AUDIO_DEVICE_NAME_CHARS)
        .collect::<String>()
        .trim()
        .to_string();
    if clean.is_empty() {
        None
    } else {
        Some(clean)
    }
}

fn device_name(device: &cpal::Device) -> Option<String> {
    device.name().ok().and_then(clean_device_name)
}

fn collect_devices(is_input: bool) -> Result<Vec<AudioDeviceSummary>, String> {
    let host = cpal::default_host();
    let default_name = if is_input {
        host.default_input_device()
            .and_then(|device| device_name(&device))
    } else {
        host.default_output_device()
            .and_then(|device| device_name(&device))
    };
    let devices = if is_input {
        host.input_devices()
            .map_err(|error| error.to_string())?
            .collect::<Vec<_>>()
    } else {
        host.output_devices()
            .map_err(|error| error.to_string())?
            .collect::<Vec<_>>()
    };

    let mut result = Vec::new();
    for device in devices {
        if result.len() >= MAX_AUDIO_DEVICES_PER_KIND {
            break;
        }
        if let Some(name) = device_name(&device) {
            if result
                .iter()
                .any(|item: &AudioDeviceSummary| item.name == name)
            {
                continue;
            }
            result.push(AudioDeviceSummary {
                id: name.clone(),
                is_default: default_name.as_deref() == Some(name.as_str()),
                name,
            });
        }
    }
    Ok(result)
}

fn find_output_device(
    host: &cpal::Host,
    requested_device_id: Option<&str>,
) -> Result<(cpal::Device, bool), String> {
    if let Some(requested_name) = requested_device_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let devices = host.output_devices().map_err(|error| error.to_string())?;
        for device in devices {
            if device_name(&device).as_deref() == Some(requested_name) {
                return Ok((device, false));
            }
        }
        return Err("The selected Meeting sound device is not available. The Windows default device was not substituted.".to_string());
    }

    host.default_output_device()
        .map(|device| (device, true))
        .ok_or_else(|| "No Windows default output device was found.".to_string())
}

#[tauri::command]
pub fn list_audio_devices() -> AudioDeviceListReport {
    let collected_input = collect_devices(true);
    let collected_output = collect_devices(false);
    let enumeration_error = match (&collected_input, &collected_output) {
        (Ok(_), Ok(_)) => String::new(),
        (Err(error), Err(other)) => format!("{error}; {other}"),
        (Err(error), _) | (_, Err(error)) => error.clone(),
    }
    .chars()
    .take(400)
    .collect::<String>();
    let input_devices = collected_input.unwrap_or_default();
    let output_devices = collected_output.unwrap_or_default();
    let ok = !input_devices.is_empty() || !output_devices.is_empty();
    AudioDeviceListReport {
        ok,
        input_devices,
        output_devices,
        blocker: if ok {
            String::new()
        } else if enumeration_error.is_empty() {
            "audio_devices:not_found".to_string()
        } else {
            format!("audio_devices:host_enumeration_failed:{enumeration_error}")
        },
        note: if !enumeration_error.is_empty() {
            format!("Native host device enumeration reported an error: {enumeration_error}")
        } else if ok {
            "Audio devices were discovered from the native host.".to_string()
        } else {
            "No audio input or output devices were discovered from the native host.".to_string()
        },
    }
}

#[tauri::command]
pub fn probe_input_device_candidate(device_id: Option<String>) -> InputPreparationStatus {
    probe_input_device_functionally(device_id.as_deref())
}

#[tauri::command]
pub fn probe_output_device_candidate(device_id: Option<String>) -> AudioDeviceProbeReport {
    let host = cpal::default_host();
    let requested_device_id = device_id
        .and_then(clean_device_name)
        .filter(|value| !value.is_empty());

    let (device, is_default) = match find_output_device(&host, requested_device_id.as_deref()) {
        Ok(value) => value,
        Err(message) => {
            return AudioDeviceProbeReport {
                ok: false,
                device_kind: "meeting_sound".to_string(),
                requested_device_id,
                resolved_device_name: None,
                is_default: false,
                sample_rate_hz: None,
                channels: None,
                blocker: "meeting_sound:device_unavailable".to_string(),
                note: message,
            }
        }
    };

    let resolved_device_name = device_name(&device);
    let config = match device.default_output_config() {
        Ok(config) => config,
        Err(error) => {
            return AudioDeviceProbeReport {
                ok: false,
                device_kind: "meeting_sound".to_string(),
                requested_device_id,
                resolved_device_name,
                is_default,
                sample_rate_hz: None,
                channels: None,
                blocker: "meeting_sound:no_output_config".to_string(),
                note: format!("The selected Meeting sound device exists, but no default output configuration was available: {error}"),
            }
        }
    };

    AudioDeviceProbeReport {
        ok: true,
        device_kind: "meeting_sound".to_string(),
        requested_device_id,
        resolved_device_name,
        is_default,
        sample_rate_hz: Some(config.sample_rate().0),
        channels: Some(config.channels()),
        blocker: String::new(),
        note: "The selected Meeting Sound output endpoint has a usable native output configuration. Actual loopback audio remains Windows runtime proof."
            .to_string(),
    }
}

#[tauri::command]
pub fn get_input_status() -> InputPreparationStatus {
    let settings = engine::load_settings();
    InputPreparationStatus::inspect_input_device(settings.audio.input_device_id.as_deref())
}


#[tauri::command]
pub fn get_audio_quality() -> AudioQualityReport {
    let status = live_audio_buffer_status();
    classify_audio_quality(
        status.has_audio,
        status.buffered_duration_ms,
        &status.evidence.reason,
        status.evidence.rms,
        status.evidence.peak,
        status.evidence.clipping_ratio,
        status.evidence.active_frame_ratio,
    )
}

#[cfg(test)]
mod audio_quality_tests {
    use super::classify_audio_quality;

    #[test]
    fn quality_requires_live_audio_before_claiming_measurement() {
        let report = classify_audio_quality(false, 0, "", 0.0, 0.0, 0.0, 0.0);
        assert!(!report.available);
        assert_eq!(report.quality, "unavailable");
    }

    #[test]
    fn quality_prioritizes_clipping_over_loudness() {
        let report = classify_audio_quality(true, 500, "", 0.3, 1.0, 0.04, 0.8);
        assert_eq!(report.quality, "clipping");
    }

    #[test]
    fn quality_surfaces_noise_like_impulses() {
        let report = classify_audio_quality(
            true,
            500,
            "rejected_noise_like_impulse",
            0.08,
            0.5,
            0.0,
            0.2,
        );
        assert_eq!(report.quality, "noisy");
    }

    #[test]
    fn quality_marks_weak_speech_as_too_quiet() {
        let report = classify_audio_quality(true, 500, "", 0.003, 0.015, 0.0, 0.04);
        assert_eq!(report.quality, "too_quiet");
    }

    #[test]
    fn quality_accepts_clear_nonclipping_speech() {
        let report = classify_audio_quality(true, 500, "", 0.08, 0.4, 0.0, 0.4);
        assert_eq!(report.quality, "good");
        assert!(report.blocker.is_empty());
    }
}
