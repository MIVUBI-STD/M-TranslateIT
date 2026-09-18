use serde_json::{json, Value};
use std::fs;

use crate::engine::runtime_state::runtime_generation_is_authoritative;

use super::functional_readiness::{
    functional_asr_output, functional_translation_output, functional_voice_actor_output_path,
    invalidate_required_outbound_ai_readiness, worker_response_value,
};
use super::{get_helper_bridge_status, send_worker_task, worker_text};

const REQUIRED_OUTBOUND_FUNCTIONAL_ID_FIXTURE: &str = "selamat pagi";

fn remove_probe_output_paths(requested_path: &str, reported_path: Option<&str>) {
    if let Some(path) = reported_path.filter(|path| !path.trim().is_empty()) {
        let _ = fs::remove_file(path);
    }
    if reported_path.map(str::trim) != Some(requested_path.trim()) && !requested_path.trim().is_empty() {
        let _ = fs::remove_file(requested_path);
    }
}

pub(super) fn run_required_outbound_ai_probe(
    meeting_generation: Option<u64>,
    output_path: &str,
) -> Result<(u64, String), &'static str> {
    if meeting_generation
        .map(|generation| generation == 0 || !runtime_generation_is_authoritative(generation))
        .unwrap_or(false)
    {
        invalidate_required_outbound_ai_readiness();
        return Err("Meeting authority");
    }
    let meeting_start_prepare = meeting_generation.is_some();

    // Refresh cheap capability truth first so a newly approved/rebuilt My Voice is
    // visible to explicit setup checks and to the generation-bound Meeting probe.
    let refreshed = send_worker_task(
        "status",
        json!({
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    if !refreshed.ok {
        invalidate_required_outbound_ai_readiness();
        return Err("local translation runtime");
    }
    let helper = get_helper_bridge_status();
    if helper.state != "ready" || !helper.provider_ready || helper.generation_token == 0 {
        invalidate_required_outbound_ai_readiness();
        return Err("local translation runtime");
    }
    let generation_token = helper.generation_token;

    let asr = send_worker_task(
        "asr_preload",
        json!({
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    if !asr.ok {
        invalidate_required_outbound_ai_readiness();
        return Err("speech recognition");
    }

    let translation = send_worker_task(
        "translate",
        json!({
            "text": REQUIRED_OUTBOUND_FUNCTIONAL_ID_FIXTURE,
            "source_language": "id",
            "target_language": "en",
            "max_new_tokens": 24,
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    let Some(translated_fixture) = functional_translation_output(&translation) else {
        invalidate_required_outbound_ai_readiness();
        return Err("Indonesian to English translation");
    };

    let actor_preflight = send_worker_task(
        "voice_actor_preflight",
        json!({
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    if !actor_preflight.ok {
        invalidate_required_outbound_ai_readiness();
        return Err("My Voice");
    }
    let Some(actor_token) = worker_text(&worker_response_value(&actor_preflight), "actor_token") else {
        invalidate_required_outbound_ai_readiness();
        return Err("My Voice");
    };

    let voice = send_worker_task(
        "voice_actor_synthesize",
        json!({
            "text": translated_fixture,
            "output_path": output_path,
            "expected_actor_token": actor_token.clone(),
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    let Some(functional_voice_path) = functional_voice_actor_output_path(&voice) else {
        remove_probe_output_paths(output_path, None);
        invalidate_required_outbound_ai_readiness();
        return Err("My Voice");
    };

    let asr_inference = send_worker_task(
        "transcribe",
        json!({
            "audio_path": functional_voice_path,
            "language": "en",
            "beam_size": 1,
            "vad_filter": false,
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    let reported_voice_path = worker_response_value(&voice)
        .get("output_path")
        .and_then(Value::as_str)
        .map(str::to_string);
    remove_probe_output_paths(output_path, reported_voice_path.as_deref());
    if !functional_asr_output(&asr_inference) {
        invalidate_required_outbound_ai_readiness();
        return Err("speech recognition");
    }

    let status = send_worker_task(
        "status",
        json!({
            "meeting_start_prepare": meeting_start_prepare,
            "meeting_generation": meeting_generation,
        }),
    );
    if !status.ok
        || worker_text(&worker_response_value(&status), "voice_actor_token").as_deref()
            != Some(actor_token.as_str())
    {
        invalidate_required_outbound_ai_readiness();
        return Err("My Voice");
    }
    let current = get_helper_bridge_status();
    if current.state != "ready"
        || !current.provider_ready
        || current.generation_token != generation_token
        || meeting_generation
            .map(|generation| !runtime_generation_is_authoritative(generation))
            .unwrap_or(false)
    {
        invalidate_required_outbound_ai_readiness();
        return Err("local translation runtime");
    }

    Ok((generation_token, actor_token))
}


#[cfg(test)]
mod tests {
    use super::remove_probe_output_paths;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("translateit-probe-{label}-{nonce}.wav"))
    }

    #[test]
    fn probe_cleanup_removes_requested_file_without_worker_report() {
        let requested = temp_path("requested");
        fs::write(&requested, b"probe").expect("write probe");

        remove_probe_output_paths(requested.to_string_lossy().as_ref(), None);

        assert!(!requested.exists());
    }

    #[test]
    fn probe_cleanup_removes_distinct_requested_and_reported_files() {
        let requested = temp_path("requested");
        let reported = temp_path("reported");
        fs::write(&requested, b"requested").expect("write requested");
        fs::write(&reported, b"reported").expect("write reported");

        remove_probe_output_paths(
            requested.to_string_lossy().as_ref(),
            Some(reported.to_string_lossy().as_ref()),
        );

        assert!(!requested.exists());
        assert!(!reported.exists());
    }
}
