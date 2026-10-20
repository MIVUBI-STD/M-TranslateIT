//! Explicit offline WAV fixture replay through the existing incoming Meeting
//! VAD/finalizer and canonical PCM16 file writer. Never a production input path.
//! Run only by explicitly invoking the ignored Rust test with serialized threads.
//! The Python harness owns fixture generation, matching identity and scoring.
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

use super::finalized_utterance::{
    clear_finalized_incoming_utterance_producer, clear_finalized_meeting_sequence,
    evicted_pending_utterance_count, observe_finalized_incoming_mono_f32_samples,
    overflow_dropped_utterance_count, reset_finalized_drop_counters,
    reset_finalized_incoming_utterance_producer, reset_finalized_meeting_sequence,
    try_take_finalized_incoming_utterance,
};
use super::live_segment_writer::write_pcm16_wav;
use super::TARGET_SAMPLE_RATE_HZ;

const RECEIPT_PREFIX: &str = "AUDIO_VAD_RECEIPT_JSON=";
const MAX_WAV_BYTES: usize = 16_000 * 125 * 2 + 44;
const FRAME_SAMPLES: usize = 16_000 / 5;

#[derive(Deserialize)]
struct Manifest {
    schema: String,
    audio_fingerprint: String,
    scenario_fingerprint: String,
    fixtures: Vec<Fixture>,
}

#[derive(Deserialize)]
struct Fixture {
    id: String,
    file: String,
    sha256: String,
}

#[derive(Deserialize)]
struct CaseSuite {
    schema: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    kind: String,
    expect: String,
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn hex64(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|item| item.is_ascii_hexdigit())
}

fn parse_pcm16_mono_16k(bytes: &[u8]) -> Result<Vec<f32>, &'static str> {
    if bytes.len() < 46 || bytes.len() > MAX_WAV_BYTES || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
    {
        return Err("invalid_riff_or_size");
    }
    // The fixture generator writes canonical RIFF PCM16: one fmt chunk and
    // immediately one data chunk. Refuse unsupported untrusted alternatives.
    if &bytes[12..16] != b"fmt " || &bytes[16..20] != 16u32.to_le_bytes().as_slice()
        || &bytes[36..40] != b"data"
        || &bytes[20..22] != 1u16.to_le_bytes().as_slice()
        || &bytes[22..24] != 1u16.to_le_bytes().as_slice()
        || &bytes[24..28] != TARGET_SAMPLE_RATE_HZ.to_le_bytes().as_slice()
        || &bytes[34..36] != 16u16.to_le_bytes().as_slice()
    {
        return Err("invalid_pcm16_layout");
    }
    let payload = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    let riff = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    if payload == 0 || payload % 2 != 0 || payload + 44 != bytes.len()
        || riff + 8 != bytes.len()
    {
        return Err("pcm_size_mismatch");
    }
    Ok(bytes[44..].chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
        .collect())
}

fn fixture_file(dir: &Path, fixture: &Fixture) -> PathBuf {
    assert!(fixture.id.len() == 3
        && fixture.id.starts_with('A')
        && fixture.id[1..].bytes().all(|ch| ch.is_ascii_digit()),
        "non-canonical scenario ID");
    let expected = format!("samples/{}.wav", fixture.id);
    assert_eq!(fixture.file, expected, "reject untrusted manifest path");
    let path = dir.join("samples").join(format!("{}.wav", fixture.id));
    assert!(!fs::symlink_metadata(&path).expect("fixture stat").file_type().is_symlink(),
        "symbolic fixture links are forbidden");
    path
}

fn stage_for_case(
    case: &Case,
    samples: &[f32],
    temp_dir: &Path,
) -> serde_json::Value {
    let session = format!("audio-public-{}", case.id);
    clear_finalized_incoming_utterance_producer();
    clear_finalized_meeting_sequence();
    reset_finalized_drop_counters();
    reset_finalized_meeting_sequence(&session);
    reset_finalized_incoming_utterance_producer(&session, TARGET_SAMPLE_RATE_HZ);

    let mut finalized = 0u32;
    let mut writer_attempts = 0u32;
    let mut writer_valid = 0u32;
    let mut speech_ms = Vec::<u64>::new();
    let mut latest_blocker = String::new();

    let mut collect = || {
        while let Some(utterance) = try_take_finalized_incoming_utterance(&session) {
            finalized += 1;
            writer_attempts += 1;
            speech_ms.push(utterance.speech_duration_ms);
            let target = temp_dir.join(format!("{}_{}.wav", case.id, writer_attempts));
            let result = write_pcm16_wav(
                &target, utterance.frame.sample_rate_hz,
                utterance.frame.channels, &utterance.frame.samples
            );
            match result {
                Ok(()) => {
                    if let Ok(metadata) = fs::metadata(&target) {
                        if metadata.len() >= 46 {
                            writer_valid += 1;
                        }
                    }
                }
                Err(error) => latest_blocker = format!("pcm16_writer:{}", error.kind()),
            }
            let _ = fs::remove_file(target);
        }
    };
    // Input is the exact hashed audio; 200-ms callback-size chunks keep
    // the existing 120-ms minimum voice evidence threshold intact.
    for frame in samples.chunks(FRAME_SAMPLES) {
        observe_finalized_incoming_mono_f32_samples(frame, TARGET_SAMPLE_RATE_HZ);
        collect();
    }
    // An explicitly disclosed quiet tail closes unfinished speech. The original
    // 61-second uninterrupted tone receives NO added silence.
    let added_quiet_ms = if case.expect == "numeric_overflow" { 0 } else { 1000 };
    if added_quiet_ms > 0 {
        let silence = vec![0.0; FRAME_SAMPLES];
        for _ in 0..added_quiet_ms / 200 {
            observe_finalized_incoming_mono_f32_samples(&silence, TARGET_SAMPLE_RATE_HZ);
            collect();
        }
    }
    let overflow = overflow_dropped_utterance_count();
    let eviction = evicted_pending_utterance_count();
    clear_finalized_incoming_utterance_producer();
    clear_finalized_meeting_sequence();
    let ok = match case.expect.as_str() {
        "no_speech" => finalized == 0 && overflow == 0,
        "numeric_overflow" => finalized == 0 && overflow >= 1,
        _ => finalized >= 1 && writer_attempts == writer_valid && eviction == 0,
    };
    json!({
        "ok": ok,
        "finalized_count": finalized,
        "overflow_count": overflow,
        "evicted_count": eviction,
        "writer_ok": writer_attempts > 0 && writer_valid == writer_attempts,
        "writer_attempts": writer_attempts,
        "writer_valid": writer_valid,
        "speech_duration_ms": speech_ms,
        "test_added_quiet_ms": added_quiet_ms,
        "test_only": true,
        "blocker": latest_blocker,
        "claim": "CANONICAL_INCOMING_VAD_AND_PCM16_WRITER_OFFLINE_ONLY"
    })
}

#[test]
#[ignore = "requires explicit, trusted public WAV fixture directory"]
fn explicit_public_audio_fixture_replay_receipts() {
    let _guard = crate::engine::runtime_state::runtime_test_guard();
    let directory = PathBuf::from(std::env::var("TRANSLATEIT_AUDIO_FIXTURE_ROOT")
        .expect("explicit TRANSLATEIT_AUDIO_FIXTURE_ROOT required"));
    assert!(directory.is_absolute() && directory.is_dir());
    let expected = std::env::var("TRANSLATEIT_AUDIO_FINGERPRINT")
        .expect("exact expected audio fingerprint required");
    assert!(hex64(&expected), "invalid audio digest");
    let data: Manifest = serde_json::from_slice(
        &fs::read(directory.join("fixtures.json")).expect("fixture manifest read")
    ).expect("valid fixture manifest");
    assert_eq!(data.schema, "translateit.audio_fixtures.v1");
    assert_eq!(data.audio_fingerprint, expected);
    assert!(hex64(&data.scenario_fingerprint));
    let cases_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tools/audio_validation/scenarios.json");
    let suite: CaseSuite = serde_json::from_slice(
        &fs::read(cases_path).expect("canonical source scenarios")
    ).expect("valid canonical audio scenarios");
    assert_eq!(suite.schema, "translateit.audio_suite.v1");
    assert_eq!(suite.cases.len(), data.fixtures.len());

    let temp_dir = std::env::temp_dir().join(format!(
        "translateit-audio-replay-{}", std::process::id()
    ));
    fs::create_dir(&temp_dir).expect("unique temporary replay directory required");
    let run = std::panic::catch_unwind(|| {
        for case in &suite.cases {
            assert!(matches!(case.kind.as_str(), "speech" | "silence" | "noise" | "tone"));
            let fixture = data.fixtures.iter().find(|item| item.id == case.id)
                .expect("missing scenario fixture");
            let bytes = fs::read(fixture_file(&directory, fixture))
                .expect("read public audio sample");
            assert_eq!(sha256_hex(&bytes), fixture.sha256, "fixture byte hash mismatch");
            let pcm = parse_pcm16_mono_16k(&bytes).expect("canonical PCM16 mono 16k fixture");
            let stage = stage_for_case(case, &pcm, &temp_dir);
            println!("{}{}", RECEIPT_PREFIX, json!({
                "schema": "translateit.audio_vad_receipt.v1",
                "scenario_id": case.id,
                "scenario_fingerprint": data.scenario_fingerprint,
                "audio_fingerprint": data.audio_fingerprint,
                "audio_sha256": fixture.sha256,
                "stage": stage,
            }));
        }
    });
    let _ = fs::remove_dir_all(&temp_dir);
    if let Err(error) = run { std::panic::resume_unwind(error); }
}
