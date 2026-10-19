use serde::Serialize;
use serde_json::{json, Value};

use crate::commands::diagnostic_trace::{
    trace_command_end, trace_command_error, trace_command_start,
};
use crate::engine::runtime_settings::load_settings;

use super::helper_bridge::{get_helper_bridge_status, send_helper_worker_task};
use super::runtime::start_helper_bridge;

const MAX_TEXT_TRANSLATION_CHARS: usize = 2_000;

#[derive(Debug, Clone, Serialize)]
pub struct TextTranslationResult {
    pub ok: bool,
    pub state: String,
    pub translated_text: String,
    pub user_message: String,
    pub blocker: String,
    pub needs_review: bool,
    pub review_hints: Vec<String>,
}

impl TextTranslationResult {
    fn success(translated_text: String, review_hints: Vec<String>) -> Self {
        Self {
            ok: true,
            state: "translated".to_string(),
            translated_text,
            user_message: if review_hints.is_empty() {
                "Translation ready.".to_string()
            } else {
                "Translation ready. Review the highlighted details before using it.".to_string()
            },
            blocker: String::new(),
            needs_review: !review_hints.is_empty(),
            review_hints,
        }
    }

    fn blocked(state: &str, user_message: &str, blocker: String) -> Self {
        Self {
            ok: false,
            state: state.to_string(),
            translated_text: String::new(),
            user_message: user_message.to_string(),
            blocker,
            needs_review: false,
            review_hints: Vec::new(),
        }
    }
}

fn clean_source(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| {
            *character != '\0'
                && !('\u{0001}'..='\u{0008}').contains(character)
                && !('\u{000b}'..='\u{001f}').contains(character)
                && *character != '\u{007f}'
        })
        .collect::<String>()
}

fn source_review_hints(source: &str) -> Vec<String> {
    // Token boundaries keep punctuation next to a critical word from hiding it,
    // without treating substrings of unrelated words as negation or references.
    let folded = source.to_lowercase();
    let words: Vec<&str> = folded
        .split(|character: char| !character.is_alphanumeric() && character != '\'')
        .filter(|word| !word.is_empty())
        .collect();
    let has_word = |choices: &[&str]| words.iter().any(|word| choices.contains(word));
    let has_pair = |first: &str, second: &str| {
        words.windows(2).any(|pair| pair == [first, second])
    };
    let mut hints = Vec::new();
    if source.chars().any(|character| character.is_ascii_digit()) {
        hints.push("Check numbers, dates, units, prices, and versions.".to_string());
    }
    if has_word(&[
        "tidak", "bukan", "jangan", "belum", "maksud", "not", "don't", "never",
        "instead", "correction",
    ]) || has_pair("do", "not") {
        hints.push("Check negation or correction wording.".to_string());
    }
    if has_word(&[
        "itu", "ini", "tersebut", "that", "this", "it", "those", "these",
    ]) || has_pair("yang", "tadi") {
        hints.push("Check references when the sentence depends on earlier context.".to_string());
    }
    hints
}

fn compact_worker_text(value: Option<&Value>) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .chars()
        .take(500)
        .collect::<String>()
}

fn worker_blocker(response: &Value) -> String {
    let blocker = compact_worker_text(response.get("blocker"));
    let note = compact_worker_text(response.get("note"));
    let model = compact_worker_text(response.get("model_id"));
    let device = compact_worker_text(response.get("device"));
    let fallback = compact_worker_text(response.get("translation_fallback_reason"));
    let chunk_index = response.get("chunk_index").and_then(Value::as_u64);
    let chunk_count = response.get("chunk_count").and_then(Value::as_u64);
    let mut parts = Vec::new();

    if !blocker.is_empty() {
        parts.push(format!("blocker={blocker}"));
    }
    if !note.is_empty() {
        parts.push(format!("note={note}"));
    }
    if !model.is_empty() {
        parts.push(format!("model={model}"));
    }
    if !device.is_empty() {
        parts.push(format!("device={device}"));
    }
    if !fallback.is_empty() {
        parts.push(format!("device_fallback={fallback}"));
    }
    if let (Some(index), Some(count)) = (chunk_index, chunk_count) {
        parts.push(format!("chunk={index}/{count}"));
    }

    if parts.is_empty() {
        "persistent helper returned no validated translation".to_string()
    } else {
        parts.join("; ")
    }
}

fn worker_generation_is_complete(response: &Value) -> bool {
    response.get("ok").and_then(Value::as_bool) == Some(true)
        && response.get("stage").and_then(Value::as_str) == Some("translate")
        && response.get("translation_contract").and_then(Value::as_str)
            == Some("canonical_bidirectional_id_en")
        && response.get("complete").and_then(Value::as_bool) == Some(true)
        && response.get("finished_with_eos").and_then(Value::as_bool) == Some(true)
}

fn worker_response_matches_direction(response: &Value, source: &str, target: &str) -> bool {
    response.get("source_language").and_then(Value::as_str) == Some(source)
        && response.get("target_language").and_then(Value::as_str) == Some(target)
}

fn worker_response_is_complete(response: &Value) -> bool {
    worker_generation_is_complete(response)
        && response.get("paragraph_structure_preserved").and_then(Value::as_bool) == Some(true)
}

fn worker_failure_message(response: &Value) -> &'static str {
    let blocker = response
        .get("blocker")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if matches!(
        blocker,
        "translation:output_hit_token_ceiling_without_eos"
            | "translation:output_ended_without_eos"
            | "translation:input_too_long_for_model"
            | "translation:chunk_count_limit_exceeded"
            | "translation:standalone_chunk_plan_empty"
    ) {
        return "This text couldn't be translated completely. Shorten the longest paragraph and try again.";
    }
    if blocker == "worker:request_deadline_expired"
        || blocker.contains("worker:response_deadline_exceeded:")
    {
        return "Translation took too long to complete. Try shorter text and translate again.";
    }
    if blocker.starts_with("helper_scheduler:wait_deadline_exceeded:text")
        || blocker.starts_with("helper_scheduler:admission_capacity_exceeded:text")
    {
        return "Text translation is busy with higher-priority Meeting work right now. Try again after the current Meeting phrase finishes.";
    }
    "Translation isn't available for this language direction right now. Check Setup or Diagnostics and try again."
}

fn ensure_persistent_helper_started() -> Result<(), Box<TextTranslationResult>> {
    let status = get_helper_bridge_status();
    if !matches!(status.state.as_str(), "not_started" | "stopped" | "error") {
        return Ok(());
    }

    let start = start_helper_bridge();
    if start.ok {
        Ok(())
    } else {
        Err(Box::new(TextTranslationResult::blocked(
            "runtime_unavailable",
            if start.state == "active_runtime_session" {
                "Text translation can't restart the local translator while Meeting or Mic Test is active. Stop the active session and try again."
            } else {
                "Local translation isn't available yet. Check Setup or Diagnostics and try again."
            },
            format!("helper_start:{}:{}", start.state, start.message),
        )))
    }
}

fn translate_with_persistent_helper(
    source: &str,
    request_kind: &str,
) -> (TextTranslationResult, String, String) {
    if let Err(result) = ensure_persistent_helper_started() {
        return (*result, String::new(), String::new());
    }

    let settings = load_settings();
    let source_language = settings.source_language.clone();
    let target_language = settings.target_language.clone();
    let payload = json!({
        "text": source,
        "source_language": settings.source_language,
        "target_language": settings.target_language,
        "request_kind": request_kind,
    });
    let response = send_helper_worker_task("translate", payload);
    let worker_response = serde_json::from_str::<Value>(&response.worker_response_json)
        .unwrap_or_else(|_| {
            json!({
                "ok": false,
                "stage": "translate",
                "blocker": "helper_bridge:invalid_translation_response",
                "note": response.message,
            })
        });
    let translated = worker_response
        .get("translated_text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();

    if response.ok
        && worker_response_is_complete(&worker_response)
        && worker_response_matches_direction(&worker_response, &source_language, &target_language)
        && !translated.is_empty()
    {
        return (
            TextTranslationResult::success(translated.to_string(), source_review_hints(source)),
            source_language,
            target_language,
        );
    }

    (
        TextTranslationResult::blocked(
            "translation_unavailable",
            worker_failure_message(&worker_response),
            worker_blocker(&worker_response),
        ),
        source_language,
        target_language,
    )
}

// Keep AI inference off the Tauri WebView event thread. The existing helper
// scheduler still owns one request, deadlines, and stale response rejection.
#[tauri::command]
pub async fn translate_text(source: String) -> Result<TextTranslationResult, String> {
    tauri::async_runtime::spawn_blocking(move || translate_text_blocking(source))
        .await
        .map_err(|_| "Text translation worker was interrupted.".to_string())
}

fn translate_text_blocking(source: String) -> TextTranslationResult {
    let started = trace_command_start(
        "translate_text",
        format!("source_chars={}", source.chars().count()),
    );
    let source = clean_source(&source);
    let result = if source.is_empty() {
        TextTranslationResult::blocked(
            "empty_input",
            "Type or paste something to translate.",
            "text_translation:empty_input".to_string(),
        )
    } else if source.chars().count() > MAX_TEXT_TRANSLATION_CHARS {
        TextTranslationResult::blocked(
            "input_too_long",
            &format!(
                "Text is too long. Limit: {MAX_TEXT_TRANSLATION_CHARS} characters. Shorten or split it and try again."
            ),
            "text_translation:input_too_long".to_string(),
        )
    } else {
        translate_with_persistent_helper(&source, "standalone_text").0
    };

    if result.ok {
        trace_command_end("translate_text", started, format!("state={}", result.state));
    } else {
        trace_command_error("translate_text", started, format!("state={}", result.state));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{
        source_review_hints, worker_failure_message, worker_generation_is_complete,
        worker_response_is_complete, worker_response_matches_direction,
    };
    use serde_json::json;

    #[test]
    fn standalone_success_requires_complete_eos_and_preserved_paragraph_structure() {
        let valid = json!({
            "ok": true,
            "stage": "translate",
            "translation_contract": "canonical_bidirectional_id_en",
            "complete": true,
            "finished_with_eos": true,
            "paragraph_structure_preserved": true,
        });
        assert!(worker_response_is_complete(&valid));

        for key in ["complete", "finished_with_eos", "paragraph_structure_preserved"] {
            let mut invalid = valid.clone();
            invalid[key] = json!(false);
            assert!(!worker_response_is_complete(&invalid));
        }
    }

    #[test]
    fn alternatives_require_verified_generation_and_direction() {
        let valid = json!({
            "ok": true, "stage": "translate",
            "translation_contract": "canonical_bidirectional_id_en",
            "complete": true, "finished_with_eos": true,
            "source_language": "id", "target_language": "en",
            "translated_text": "Please review it.",
        });
        assert!(worker_generation_is_complete(&valid));
        assert!(worker_response_matches_direction(&valid, "id", "en"));
        assert!(!worker_response_matches_direction(&valid, "en", "id"));
        for field in ["ok", "complete", "finished_with_eos"] {
            let mut broken = valid.clone();
            broken[field] = json!(false);
            assert!(!worker_generation_is_complete(&broken));
        }
        let mut missing_contract = valid.clone();
        missing_contract["translation_contract"] = json!("unknown");
        assert!(!worker_generation_is_complete(&missing_contract));
    }

    #[test]
    fn review_cues_recognize_punctuation_without_embedded_words() {
        let negation = source_review_hints("Tidak, jangan ubah angka 21.");
        assert!(negation.iter().any(|hint| hint.contains("negation")));
        assert!(negation.iter().any(|hint| hint.contains("numbers")));
        let corrections = source_review_hints("No, do not change it!");
        assert!(corrections.iter().any(|hint| hint.contains("negation")));
        assert!(corrections.iter().any(|hint| hint.contains("references")));
        assert!(!source_review_hints("The notebook is operational.").iter()
            .any(|hint| hint.contains("negation")));
        assert!(source_review_hints("Kita membahas yang tadi.").iter()
            .any(|hint| hint.contains("references")));
    }

    #[test]
    fn incomplete_generation_has_product_level_recovery_copy() {
        let response = json!({
            "blocker": "translation:output_hit_token_ceiling_without_eos"
        });
        assert_eq!(
            worker_failure_message(&response),
            "This text couldn't be translated completely. Shorten the longest paragraph and try again."
        );
    }

    #[test]
    fn transport_deadline_has_timeout_recovery_copy() {
        let response = json!({
            "blocker": "helper_bridge:translate_read_failed:worker:response_deadline_exceeded:180000ms"
        });
        assert_eq!(
            worker_failure_message(&response),
            "Translation took too long to complete. Try shorter text and translate again."
        );
    }

    #[test]
    fn text_scheduler_wait_has_busy_recovery_copy() {
        let response = json!({
            "blocker": "helper_scheduler:wait_deadline_exceeded:text:15000ms"
        });
        assert_eq!(
            worker_failure_message(&response),
            "Text translation is busy with higher-priority Meeting work right now. Try again after the current Meeting phrase finishes."
        );
    }
}
