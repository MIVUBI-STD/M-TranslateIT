use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::meeting_output::deliver_meeting_output_wav;
use crate::engine::runtime_state::{
    begin_application_meeting_session, clear_runtime_session_state,
};

fn unique_test_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "translateit-meeting-output-{label}-{}-{nonce}.wav",
        std::process::id()
    ))
}

fn pcm16_wav(sample_rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
    let data_bytes = (samples.len() * 2) as u32;
    let block_align = channels.saturating_mul(2);
    let byte_rate = sample_rate.saturating_mul(u32::from(block_align));
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36u32.saturating_add(data_bytes)).to_le_bytes());
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

fn authoritative_generation() -> u64 {
    let _ = clear_runtime_session_state();
    let report = begin_application_meeting_session();
    assert!(report.blocker.is_empty(), "{}", report.blocker);
    report
        .snapshot
        .expect("Meeting claim should expose an authoritative snapshot")
        .generation
}

fn write_fixture(label: &str, bytes: &[u8]) -> PathBuf {
    let path = unique_test_path(label);
    fs::write(&path, bytes).expect("write Meeting output WAV fixture");
    path
}

fn assert_parser_blocker(label: &str, bytes: &[u8], expected_blocker: &str) {
    let generation = authoritative_generation();
    let path = write_fixture(label, bytes);
    let report = deliver_meeting_output_wav(
        path.to_string_lossy().as_ref(),
        Some("unused-test-output"),
        generation,
    );
    assert!(!report.ok);
    assert!(!report.execution_attempted);
    assert_eq!(report.blocker, expected_blocker);
    assert!(report.stream_build_ms.is_none());
    assert!(report.stream_start_to_first_audio_ms.is_none());
    let _ = fs::remove_file(path);
    let _ = clear_runtime_session_state();
}

#[test]
fn stale_generation_is_rejected_before_file_or_device_access() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    let _ = clear_runtime_session_state();
    let report = deliver_meeting_output_wav(
        "definitely-missing-output.wav",
        Some("unused-test-output"),
        u64::MAX,
    );
    assert!(!report.ok);
    assert!(!report.execution_attempted);
    assert_eq!(report.blocker, "meeting_output:generation_not_authoritative");
}

#[test]
fn malformed_wav_contracts_fail_before_device_access() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    assert_parser_blocker(
        "invalid-riff",
        b"not a wav",
        "meeting_output:wav_invalid_riff",
    );

    let base = pcm16_wav(16_000, 1, &[0, 1]);

    let mut missing_fmt = base.clone();
    missing_fmt[12..16].copy_from_slice(b"JUNK");
    assert_parser_blocker(
        "missing-fmt",
        &missing_fmt,
        "meeting_output:wav_fmt_missing",
    );

    let mut missing_data = base.clone();
    missing_data[36..40].copy_from_slice(b"JUNK");
    assert_parser_blocker(
        "missing-data",
        &missing_data,
        "meeting_output:wav_data_missing",
    );

    let mut truncated = base.clone();
    truncated[40..44].copy_from_slice(&100u32.to_le_bytes());
    assert_parser_blocker(
        "truncated-data",
        &truncated,
        "meeting_output:wav_truncated",
    );

    let zero_channels = pcm16_wav(16_000, 0, &[0]);
    assert_parser_blocker(
        "zero-channels",
        &zero_channels,
        "meeting_output:wav_channels_unsupported",
    );

    let low_rate = pcm16_wav(7_999, 1, &[0]);
    assert_parser_blocker(
        "low-rate",
        &low_rate,
        "meeting_output:wav_sample_rate_unsupported",
    );

    let misaligned = pcm16_wav(16_000, 2, &[0]);
    assert_parser_blocker(
        "misaligned-frame",
        &misaligned,
        "meeting_output:wav_frame_alignment_invalid",
    );

    let mut invalid_layout = base.clone();
    invalid_layout[34..36].copy_from_slice(&24u16.to_le_bytes());
    assert_parser_blocker(
        "invalid-layout",
        &invalid_layout,
        "meeting_output:wav_sample_layout_invalid",
    );

    let mut unsupported_encoding = base.clone();
    unsupported_encoding[20..22].copy_from_slice(&6u16.to_le_bytes());
    assert_parser_blocker(
        "unsupported-encoding",
        &unsupported_encoding,
        "meeting_output:wav_encoding_unsupported",
    );

    let empty = pcm16_wav(16_000, 1, &[]);
    assert_parser_blocker("empty", &empty, "meeting_output:wav_empty");
}

#[test]
fn valid_pcm16_reaches_prepared_device_boundary_without_execution() {
    let _serial = crate::engine::runtime_state::runtime_test_guard();
    let generation = authoritative_generation();
    let wav = pcm16_wav(16_000, 1, &[0, 8_000, -8_000, 0]);
    let path = write_fixture("valid-pcm16", &wav);

    let report = deliver_meeting_output_wav(
        path.to_string_lossy().as_ref(),
        Some("unused-test-output"),
        generation,
    );

    assert!(!report.ok);
    assert!(!report.execution_attempted);
    assert_eq!(report.blocker, "meeting_output:prepared_output_device_missing");

    let _ = fs::remove_file(path);
    let _ = clear_runtime_session_state();
}
