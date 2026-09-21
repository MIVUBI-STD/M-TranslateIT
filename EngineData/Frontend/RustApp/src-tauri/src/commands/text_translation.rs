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
    let folded = format!(" {} ", source.to_lowercase());
    let mut hints = Vec::new();
    if source.chars().any(|character| character.is_ascii_digit()) {
        hints.push("Check numbers, dates, units, prices, and versions.".to_string());
    }
    if [
        " tidak ", " bukan ", " jangan ", " belum ", " maksud ", " not ", " don't ",
        " do not ", " never ", " instead ", " correction ",
    ]
    .iter()
    .any(|needle| folded.contains(needle))
    {
        hints.push("Check negation or correction wording.".to_string());
    }
    if [" itu ", " ini ", " yang tadi ", " tersebut ", " that ", " this ", " it ", " those ", " these "]
        .iter()
        .any(|needle| folded.contains(needle))
    {
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

fn worker_response_is_complete(response: &Value) -> bool {
    response.get("stage").and_then(Value::as_str) == Some("translate")
        && response
            .get("translation_contract")
            .and_then(Value::as_str)
            == Some("canonical_bidirectional_id_en")
        && response.get("complete").and_then(Value::as_bool) == Some(true)
        && response.get("finished_with_eos").and_then(Value::as_bool) == Some(true)
        && response
            .get("paragraph_structure_preserved")
            .and_then(Value::as_bool)
            == Some(true)
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

fn ensure_persistent_helper_started() -> Result<(), TextTranslationResult> {
    let status = get_helper_bridge_status();
    if !matches!(status.state.as_str(), "not_started" | "stopped" | "error") {
        return Ok(());
    }

    let start = start_helper_bridge();
    if start.ok {
        Ok(())
    } else {
        Err(TextTranslationResult::blocked(
            "runtime_unavailable",
            if start.state == "active_runtime_session" {
                "Text translation can't restart the local translator while Meeting or Mic Test is active. Stop the active session and try again."
            } else {
                "Local translation isn't available yet. Check Setup or Diagnostics and try again."
            },
            format!("helper_start:{}:{}", start.state, start.message),
        ))
    }
}

fn translate_with_persistent_helper(source: &str) -> TextTranslationResult {
    if let Err(result) = ensure_persistent_helper_started() {
        return result;
    }

    let settings = load_settings();
    let payload = json!({
        "text": source,
        "source_language": settings.source_language,
        "target_language": settings.target_language,
        "request_kind": "standalone_text",
        "translation_style": settings.translation_style,
        "terminology": &settings.terminology,
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

    if response.ok && worker_response_is_complete(&worker_response) && !translated.is_empty() {
        return TextTranslationResult::success(translated.to_string(), source_review_hints(source));
    }

    TextTranslationResult::blocked(
        "translation_unavailable",
        worker_failure_message(&worker_response),
        worker_blocker(&worker_response),
    )
}

#[tauri::command]
pub fn translate_text(source: String) -> TextTranslationResult {
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
        translate_with_persistent_helper(&source)
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
    use super::{worker_failure_message, worker_response_is_complete};
    use serde_json::json;

    #[test]
    fn standalone_success_requires_complete_eos_and_preserved_paragraph_structure() {
        let valid = json!({
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


const MAX_ALTERNATIVE_SOURCE_CHARS: usize = 1_000;

#[tauri::command]
pub fn translate_text_alternative(source: String, current_translation: String) -> TextTranslationResult {
    let started = trace_command_start(
        "translate_text_alternative",
        format!("source_chars={}", source.chars().count()),
    );
    let source = clean_source(&source);
    let current_translation = clean_source(&current_translation);
    let result = if source.is_empty() || current_translation.is_empty() {
        TextTranslationResult::blocked(
            "alternative_unavailable",
            "Translate the text first, then request another wording.",
            "text_translation:alternative_requires_current_translation".to_string(),
        )
    } else if source.chars().count() > MAX_ALTERNATIVE_SOURCE_CHARS {
        TextTranslationResult::blocked(
            "alternative_input_too_long",
            "Another wording is available for shorter text only. Shorten the source or edit the current translation.",
            "text_translation:alternative_source_too_long".to_string(),
        )
    } else if let Err(result) = ensure_persistent_helper_started() {
        result
    } else {
        let settings = load_settings();
        let payload = json!({
            "text": source,
            "source_language": settings.source_language,
            "target_language": settings.target_language,
            "request_kind": "standalone_alternative",
            "translation_style": settings.translation_style,
            "alternative_of": current_translation,
            "terminology": &settings.terminology,
        });
        let response = send_helper_worker_task("translate", payload);
        let worker_response = serde_json::from_str::<Value>(&response.worker_response_json)
            .unwrap_or_else(|_| json!({
                "ok": false,
                "stage": "translate",
                "blocker": "helper_bridge:invalid_translation_response",
                "note": response.message,
            }));
        let translated = worker_response
            .get("translated_text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if response.ok
            && worker_response.get("complete").and_then(Value::as_bool) == Some(true)
            && worker_response.get("finished_with_eos").and_then(Value::as_bool) == Some(true)
            && !translated.is_empty()
            && translated != current_translation
        {
            TextTranslationResult::success(translated.to_string(), source_review_hints(&source))
        } else if translated == current_translation {
            TextTranslationResult::blocked(
                "no_distinct_alternative",
                "The local translator didn't find a meaning-preserving alternative. Keep or edit the current translation.",
                "text_translation:no_distinct_alternative".to_string(),
            )
        } else {
            TextTranslationResult::blocked(
                "alternative_unavailable",
                worker_failure_message(&worker_response),
                worker_blocker(&worker_response),
            )
        }
    };

    if result.ok {
        trace_command_end("translate_text_alternative", started, format!("state={}", result.state));
    } else {
        trace_command_error("translate_text_alternative", started, format!("state={}", result.state));
    }
    result
}
