use serde_json::json;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_settings::load_settings;

use super::committed_turns::{
    commit_meeting_turn, recent_outbound_context_pairs, update_committed_turn_delivery_state,
    update_committed_turn_outbound_timing,
};
use super::super::helper_bridge::{required_outbound_voice_actor_token, send_helper_worker_task};
use super::{
    generation_is_live, worker_blocker, worker_number, worker_text, MeetingOutboundProcessResult,
};
use super::playback_runtime::{enqueue_meeting_playback, PreparedPlaybackJob};
use super::session_state::{
    elapsed_millis, set_outbound_timing, update_outbound_status, OutboundTimingContext,
};

fn tts_output_path(session_id: &str, generation: u64, event_sequence: u64) -> String {
    format!(
        "UserData/CacheData/meeting_tts/{}_g{}_s{}.wav",
        session_id, generation, event_sequence
    )
}

fn remove_temporary_tts(path: &str) {
    if !path.trim().is_empty() {
        let _ = fs::remove_file(path);
    }
}

pub(super) fn remove_temporary_tts_paths(requested_path: &str, reported_path: &str) {
    remove_temporary_tts(reported_path);
    if requested_path.trim() != reported_path.trim() {
        remove_temporary_tts(requested_path);
    }
}

pub(super) fn cleanup_meeting_tts_for_session(
    session_id: &str,
    generation: u64,
) -> Result<usize, String> {
    let project_paths = ProjectPaths::discover();
    let tts_dir = PathBuf::from(project_paths.user_cache_dir).join("meeting_tts");
    cleanup_meeting_tts_for_session_in_dir(&tts_dir, session_id, generation)
}

fn cleanup_meeting_tts_for_session_in_dir(
    tts_dir: &Path,
    session_id: &str,
    generation: u64,
) -> Result<usize, String> {
    if session_id.trim().is_empty()
        || session_id.contains('/')
        || session_id.contains('\\')
        || !session_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err("meeting_tts_cleanup:invalid_session_id".to_string());
    }
    let prefix = format!("{session_id}_g{generation}_s");
    let entries = match fs::read_dir(tts_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(_) => return Err("meeting_tts_cleanup:read_dir_failed".to_string()),
    };
    let mut removed = 0usize;
    for entry in entries {
        let entry = entry.map_err(|_| "meeting_tts_cleanup:read_entry_failed".to_string())?;
        let file_type = entry
            .file_type()
            .map_err(|_| "meeting_tts_cleanup:file_type_failed".to_string())?;
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&prefix) || !name.ends_with(".wav") {
            continue;
        }
        fs::remove_file(entry.path()).map_err(|_| "meeting_tts_cleanup:remove_failed".to_string())?;
        removed = removed.saturating_add(1);
    }
    Ok(removed)
}

fn stale_outbound_result(
    generation: u64,
    session_id: &str,
    event_sequence: u64,
    utterance_id: u64,
) -> MeetingOutboundProcessResult {
    let _ =
        update_committed_turn_delivery_state(session_id, generation, utterance_id, "interrupted");
    MeetingOutboundProcessResult {
        ok: false,
        delivered: false,
        state: "stale_generation".to_string(),
        blocker: "meeting_outbound:generation_not_authoritative".to_string(),
        note: "Outbound work was discarded because its Meeting generation no longer owns output authority."
            .to_string(),
        generation,
        utterance_sequence: event_sequence,
        runtime_claim: "meeting_outbound_generation_rejected_before_promotion".to_string(),
    }
}

pub(super) fn process_outbound_wav(
    generation: u64,
    session_id: &str,
    event_sequence: u64,
    utterance_id: u64,
    speech_duration_ms: u64,
    audio_path: String,
    mut timing: OutboundTimingContext,
) -> MeetingOutboundProcessResult {
    if !generation_is_live(generation) {
        return stale_outbound_result(generation, session_id, event_sequence, utterance_id);
    }

    let settings = load_settings();
    let asr_hotwords = settings.asr_hotwords();

    update_outbound_status(
        generation,
        session_id,
        "transcribing",
        event_sequence,
        false,
        true,
        "",
        "Finalized Indonesian speech is being transcribed locally.",
    );
    set_outbound_timing(generation, session_id, event_sequence, &timing.metrics);
    let asr_started_at = Instant::now();
    let asr = send_helper_worker_task(
        "transcribe",
        json!({
            "audio_path": audio_path,
            "language": "id",
            "beam_size": 1,
            "vad_filter": true,
            "hotwords": asr_hotwords,
            "meeting_session_id": session_id,
            "meeting_lane": "you",
            "meeting_generation": generation,
            "meeting_sequence": event_sequence,
            "utterance_id": utterance_id,
            "source_speech_duration_ms": speech_duration_ms,
        }),
    );
    timing.metrics.asr_ms = Some(elapsed_millis(asr_started_at, Instant::now()));
    set_outbound_timing(generation, session_id, event_sequence, &timing.metrics);
    if !generation_is_live(generation) {
        return stale_outbound_result(generation, session_id, event_sequence, utterance_id);
    }
    let transcript = worker_text(&asr, "transcript_text");
    if !asr.ok || transcript.is_none() {
        let blocker = worker_blocker(&asr, "asr:empty_transcript");
        let empty = blocker.contains("empty_transcript");
        update_outbound_status(
            generation,
            session_id,
            if empty {
                "listening"
            } else {
                "attention_needed"
            },
            event_sequence,
            false,
            empty,
            if empty { "" } else { &blocker },
            if empty {
                "Finalized speech did not produce a stable transcript. No Meeting output was generated."
            } else {
                "Local ASR failed before translation. No Meeting output was generated."
            },
        );
        return MeetingOutboundProcessResult {
            ok: empty,
            delivered: false,
            state: if empty {
                "no_stable_transcript"
            } else {
                "asr_failed"
            }
            .to_string(),
            blocker: if empty { String::new() } else { blocker },
            note: "No Meeting output was generated from this finalized segment.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_finalized_segment_not_delivered".to_string(),
        };
    }
    let transcript = transcript.unwrap_or_default();

    update_outbound_status(
        generation,
        session_id,
        "translating",
        event_sequence,
        false,
        true,
        "",
        "Final Indonesian transcript is being translated to English.",
    );
    let translation_started_at = Instant::now();
    let context_pairs = recent_outbound_context_pairs(session_id, 3);
    let terminology = settings.terminology;
    let translation = send_helper_worker_task(
        "translate",
        json!({
            "text": transcript.clone(),
            "source_language": "id",
            "target_language": "en",
            "max_new_tokens": 96,
            "translation_style": settings.translation_style,
            "context_pairs": context_pairs,
            "terminology": terminology,
            "meeting_session_id": session_id,
            "meeting_lane": "you",
            "meeting_generation": generation,
            "meeting_sequence": event_sequence,
            "utterance_id": utterance_id,
        }),
    );
    timing.metrics.translation_ms = Some(elapsed_millis(translation_started_at, Instant::now()));
    timing.metrics.translation_tokenization_ms = worker_number(&translation, "tokenization_ms");
    timing.metrics.translation_inference_ms = worker_number(&translation, "inference_ms");
    timing.metrics.translation_decode_ms = worker_number(&translation, "decode_ms");
    timing.metrics.translation_tokens_per_second =
        worker_number(&translation, "inference_tokens_per_second");
    set_outbound_timing(generation, session_id, event_sequence, &timing.metrics);
    if !generation_is_live(generation) {
        return stale_outbound_result(generation, session_id, event_sequence, utterance_id);
    }
    let translated_text = worker_text(&translation, "translated_text");
    if !translation.ok || translated_text.is_none() {
        let blocker = worker_blocker(&translation, "translation:empty_output");
        update_outbound_status(
            generation,
            session_id,
            "attention_needed",
            event_sequence,
            false,
            false,
            &blocker,
            "Local translation failed before TTS. No Meeting output was generated.",
        );
        return MeetingOutboundProcessResult {
            ok: false,
            delivered: false,
            state: "translation_failed".to_string(),
            blocker,
            note: "No Meeting output was generated from this finalized segment.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_translation_failed_before_output".to_string(),
        };
    }
    let translated_text = translated_text.unwrap_or_default();

    let _ = commit_meeting_turn(
        session_id,
        event_sequence,
        Some(generation),
        utterance_id,
        "you",
        "id",
        "en",
        &transcript,
        &translated_text,
        Some("preparing_voice"),
        Some(timing.metrics.clone()),
    );

    update_outbound_status(
        generation,
        session_id,
        "synthesizing",
        event_sequence,
        false,
        true,
        "",
        "Translated English text is being synthesized locally.",
    );
    let Some(actor_token) = required_outbound_voice_actor_token(generation) else {
        let blocker = "voice_actor:meeting_actor_authority_missing".to_string();
        let _ = update_committed_turn_delivery_state(
            session_id,
            generation,
            utterance_id,
            "output_failed",
        );
        update_outbound_status(
            generation,
            session_id,
            "attention_needed",
            event_sequence,
            false,
            false,
            &blocker,
            "Meeting voice authority is no longer bound to this Meeting generation. Stop and start Translation again before producing more voice output.",
        );
        return MeetingOutboundProcessResult {
            ok: false,
            delivered: false,
            state: "tts_failed".to_string(),
            blocker,
            note: "No Meeting output was generated from this finalized segment.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_voice_actor_authority_missing".to_string(),
        };
    };
    let requested_tts_path = tts_output_path(session_id, generation, event_sequence);
    let tts_started_at = Instant::now();
    let tts = send_helper_worker_task(
        "voice_actor_synthesize",
        json!({
            "text": translated_text.clone(),
            "source_text": transcript.clone(),
            "source_speech_duration_ms": speech_duration_ms,
            "output_path": requested_tts_path,
            "expected_actor_token": actor_token,
            "meeting_session_id": session_id,
            "meeting_lane": "you",
            "meeting_generation": generation,
            "meeting_sequence": event_sequence,
            "utterance_id": utterance_id,
        }),
    );
    timing.metrics.tts_ms = Some(elapsed_millis(tts_started_at, Instant::now()));
    set_outbound_timing(generation, session_id, event_sequence, &timing.metrics);
    let _ = update_committed_turn_outbound_timing(
        session_id,
        generation,
        utterance_id,
        &timing.metrics,
    );
    let tts_path = worker_text(&tts, "output_path").unwrap_or_default();
    if !generation_is_live(generation) {
        remove_temporary_tts_paths(&requested_tts_path, &tts_path);
        return stale_outbound_result(generation, session_id, event_sequence, utterance_id);
    }
    if !tts.ok || tts_path.is_empty() {
        let blocker = worker_blocker(&tts, "voice_actor:missing_output");
        remove_temporary_tts_paths(&requested_tts_path, &tts_path);
        let _ = update_committed_turn_delivery_state(
            session_id,
            generation,
            utterance_id,
            "output_failed",
        );
        update_outbound_status(
            generation,
            session_id,
            "attention_needed",
            event_sequence,
            false,
            false,
            &blocker,
            "Meeting voice synthesis failed before Meeting delivery. No Meeting output was generated.",
        );
        return MeetingOutboundProcessResult {
            ok: false,
            delivered: false,
            state: "tts_failed".to_string(),
            blocker,
            note: "No Meeting output was generated from this finalized segment.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_tts_failed_before_output".to_string(),
        };
    }

    let playback_job = PreparedPlaybackJob {
        generation,
        session_id: session_id.to_string(),
        event_sequence,
        utterance_id,
        requested_tts_path,
        actual_tts_path: tts_path,
        timing,
        prepared_at: Instant::now(),
    };

    let _ = update_committed_turn_delivery_state(
        session_id,
        generation,
        utterance_id,
        "queued",
    );
    update_outbound_status(
        generation,
        session_id,
        "queued",
        event_sequence,
        false,
        true,
        "",
        "Translated voice is prepared and queued for bounded Meeting playback.",
    );

    if let Err(blocker) = enqueue_meeting_playback(playback_job) {
        let stale = blocker == "meeting_playback:generation_not_authoritative";
        let _ = update_committed_turn_delivery_state(
            session_id,
            generation,
            utterance_id,
            if stale { "interrupted" } else { "output_failed" },
        );
        update_outbound_status(
            generation,
            session_id,
            if stale { "listening" } else { "attention_needed" },
            event_sequence,
            false,
            stale,
            if stale { "" } else { &blocker },
            if stale {
                "Prepared voice output was discarded because its Meeting generation lost authority before playback."
            } else {
                "Prepared voice output could not enter the bounded Meeting playback runtime."
            },
        );
        return MeetingOutboundProcessResult {
            ok: stale,
            delivered: false,
            state: if stale { "interrupted" } else { "playback_enqueue_failed" }.to_string(),
            blocker: if stale { String::new() } else { blocker },
            note: "No additional playback attempt was made.".to_string(),
            generation,
            utterance_sequence: event_sequence,
            runtime_claim: "meeting_outbound_prepared_output_not_delivered".to_string(),
        };
    }

    MeetingOutboundProcessResult {
        ok: true,
        delivered: false,
        state: "queued".to_string(),
        blocker: String::new(),
        note: "Local ASR, translation, and voice synthesis completed; bounded playback now owns delivery."
            .to_string(),
        generation,
        utterance_sequence: event_sequence,
        runtime_claim: "meeting_outbound_prepared_and_handed_to_bounded_playback".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cleanup_meeting_tts_for_session_in_dir, remove_temporary_tts_paths, tts_output_path,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_wav(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("translateit-{label}-{nonce}.wav"))
    }

    #[test]
    fn tts_output_path_is_generation_and_event_scoped() {
        assert_eq!(
            tts_output_path("session-a", 7, 11),
            "UserData/CacheData/meeting_tts/session-a_g7_s11.wav"
        );
    }

    #[test]
    fn session_tts_cleanup_removes_only_matching_generation_files() {
        let root = temp_wav("session-cleanup-root").with_extension("");
        fs::create_dir_all(&root).expect("create tts cleanup root");
        fs::write(root.join("meeting_100_7_g9_s1.wav"), b"a").expect("write matching tts");
        fs::write(root.join("meeting_100_7_g9_s2.wav"), b"b").expect("write matching tts");
        fs::write(root.join("meeting_100_7_g8_s1.wav"), b"c").expect("write old generation");
        fs::write(root.join("other_g9_s1.wav"), b"d").expect("write foreign session");

        let removed = cleanup_meeting_tts_for_session_in_dir(&root, "meeting_100_7", 9)
            .expect("cleanup session tts");

        assert_eq!(removed, 2);
        assert!(!root.join("meeting_100_7_g9_s1.wav").exists());
        assert!(!root.join("meeting_100_7_g9_s2.wav").exists());
        assert!(root.join("meeting_100_7_g8_s1.wav").exists());
        assert!(root.join("other_g9_s1.wav").exists());

        fs::remove_dir_all(root).expect("remove tts cleanup root");
    }

    #[test]
    fn tts_cleanup_removes_requested_path_when_worker_response_has_no_path() {
        let requested = temp_wav("requested");
        fs::write(&requested, b"temporary").expect("write temporary wav");

        remove_temporary_tts_paths(requested.to_string_lossy().as_ref(), "");

        assert!(!requested.exists());
    }

    #[test]
    fn tts_cleanup_removes_distinct_requested_and_reported_paths() {
        let requested = temp_wav("requested");
        let reported = temp_wav("reported");
        fs::write(&requested, b"requested").expect("write requested");
        fs::write(&reported, b"reported").expect("write reported");

        remove_temporary_tts_paths(
            requested.to_string_lossy().as_ref(),
            reported.to_string_lossy().as_ref(),
        );

        assert!(!requested.exists());
        assert!(!reported.exists());
    }
}
