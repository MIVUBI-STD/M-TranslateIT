use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::paths::ProjectPaths;

use super::committed_turns::exportable_committed_turn_snapshot;
use super::MeetingCommittedTurnsSnapshot;

#[derive(Debug, Clone, Serialize)]
pub struct MeetingTranscriptExportStatus {
    pub available: bool,
    pub session_id: Option<String>,
    pub turn_count: usize,
    pub truncated: bool,
    pub dropped_turn_count: u64,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingTranscriptExportResult {
    pub ok: bool,
    pub format: String,
    pub file_path: Option<String>,
    pub turn_count: usize,
    pub truncated: bool,
    pub message: String,
    pub blocker: String,
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn sanitize_component(value: &str) -> String {
    let cleaned = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .take(48)
        .collect::<String>();
    if cleaned.is_empty() {
        "meeting".to_string()
    } else {
        cleaned
    }
}

fn language_label(value: &str) -> &str {
    match value {
        "id" => "Indonesian",
        "en" => "English",
        _ => value,
    }
}

fn format_markdown(snapshot: &MeetingCommittedTurnsSnapshot) -> String {
    let mut out = String::from("# TranslateIT Meeting Transcript\n\n");
    if let Some(session_id) = snapshot.session_id.as_deref() {
        out.push_str(&format!("Session: {}\n\n", session_id));
    }
    if snapshot.truncated {
        out.push_str(&format!(
            "> Earlier transcript content was not retained. {} committed turns were dropped from the bounded live buffer.\n\n",
            snapshot.dropped_turn_count
        ));
    }
    for turn in &snapshot.turns {
        let speaker = if turn.lane == "you" { "You" } else { "Them" };
        out.push_str(&format!(
            "## {speaker} · {} → {}\n\n**Source**\n\n{}\n\n**Translation**\n\n{}\n\n",
            language_label(&turn.source_language),
            language_label(&turn.target_language),
            turn.source_text,
            turn.translated_text,
        ));
    }
    out
}

fn format_text(snapshot: &MeetingCommittedTurnsSnapshot) -> String {
    let mut out = String::from("TranslateIT Meeting Transcript\n");
    if let Some(session_id) = snapshot.session_id.as_deref() {
        out.push_str(&format!("Session: {session_id}\n"));
    }
    if snapshot.truncated {
        out.push_str(&format!(
            "WARNING: {} earlier committed turns were not retained in the bounded live transcript.\n",
            snapshot.dropped_turn_count
        ));
    }
    out.push('\n');
    for turn in &snapshot.turns {
        let speaker = if turn.lane == "you" { "YOU" } else { "THEM" };
        out.push_str(&format!(
            "[{speaker}] {} -> {}\nSource: {}\nTranslation: {}\n\n",
            language_label(&turn.source_language),
            language_label(&turn.target_language),
            turn.source_text,
            turn.translated_text,
        ));
    }
    out
}

pub(super) fn transcript_export_status() -> MeetingTranscriptExportStatus {
    let Some(snapshot) = exportable_committed_turn_snapshot() else {
        return MeetingTranscriptExportStatus {
            available: false,
            session_id: None,
            turn_count: 0,
            truncated: false,
            dropped_turn_count: 0,
            note: "No active or recently ended Meeting transcript is available to export.".to_string(),
        };
    };

    MeetingTranscriptExportStatus {
        available: !snapshot.turns.is_empty(),
        session_id: snapshot.session_id.clone(),
        turn_count: snapshot.turns.len(),
        truncated: snapshot.truncated,
        dropped_turn_count: snapshot.dropped_turn_count,
        note: "Transcript remains transient until you explicitly export it.".to_string(),
    }
}

pub(super) fn export_transcript(format: &str) -> MeetingTranscriptExportResult {
    let normalized = match format.trim().to_lowercase().as_str() {
        "txt" => "txt",
        "md" | "markdown" => "md",
        _ => {
            return MeetingTranscriptExportResult {
                ok: false,
                format: format.to_string(),
                file_path: None,
                turn_count: 0,
                truncated: false,
                message: "Choose Markdown or TXT for transcript export.".to_string(),
                blocker: "meeting_transcript_export:unsupported_format".to_string(),
            };
        }
    };

    let Some(snapshot) = exportable_committed_turn_snapshot() else {
        return MeetingTranscriptExportResult {
            ok: false,
            format: normalized.to_string(),
            file_path: None,
            turn_count: 0,
            truncated: false,
            message: "There is no Meeting transcript available to export.".to_string(),
            blocker: "meeting_transcript_export:no_transcript".to_string(),
        };
    };

    if snapshot.turns.is_empty() {
        return MeetingTranscriptExportResult {
            ok: false,
            format: normalized.to_string(),
            file_path: None,
            turn_count: 0,
            truncated: snapshot.truncated,
            message: "There are no committed translated phrases to export yet.".to_string(),
            blocker: "meeting_transcript_export:no_turns".to_string(),
        };
    }

    let paths = ProjectPaths::discover();
    let destination = PathBuf::from(&paths.user_saved_dir).join("Transcripts");
    if fs::create_dir_all(&destination).is_err() {
        return MeetingTranscriptExportResult {
            ok: false,
            format: normalized.to_string(),
            file_path: None,
            turn_count: snapshot.turns.len(),
            truncated: snapshot.truncated,
            message: "Transcript export folder could not be created.".to_string(),
            blocker: "meeting_transcript_export:create_dir_failed".to_string(),
        };
    }

    let session = sanitize_component(snapshot.session_id.as_deref().unwrap_or("meeting"));
    let filename = format!("translateit_{session}_{}.{}", unix_ms(), normalized);
    let path = destination.join(filename);
    let body = if normalized == "md" {
        format_markdown(&snapshot)
    } else {
        format_text(&snapshot)
    };

    if fs::write(&path, body.as_bytes()).is_err() {
        return MeetingTranscriptExportResult {
            ok: false,
            format: normalized.to_string(),
            file_path: None,
            turn_count: snapshot.turns.len(),
            truncated: snapshot.truncated,
            message: "Transcript could not be written to disk.".to_string(),
            blocker: "meeting_transcript_export:write_failed".to_string(),
        };
    }

    MeetingTranscriptExportResult {
        ok: true,
        format: normalized.to_string(),
        file_path: Some(path.to_string_lossy().replace('\\', "/")),
        turn_count: snapshot.turns.len(),
        truncated: snapshot.truncated,
        message: format!(
            "Transcript exported as {} with {} committed phrases.",
            normalized.to_uppercase(),
            snapshot.turns.len()
        ),
        blocker: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{format_markdown, format_text};
    use crate::commands::meeting_session::{MeetingCommittedTurn, MeetingCommittedTurnsSnapshot};

    fn snapshot() -> MeetingCommittedTurnsSnapshot {
        MeetingCommittedTurnsSnapshot {
            ok: true,
            has_session: false,
            session_id: Some("session-a".to_string()),
            turns: vec![MeetingCommittedTurn {
                session_id: "session-a".to_string(),
                sequence: 1,
                generation: None,
                utterance_id: 1,
                lane: "incoming".to_string(),
                source_language: "en".to_string(),
                target_language: "id".to_string(),
                source_text: "See you tomorrow.".to_string(),
                translated_text: "Sampai jumpa besok.".to_string(),
                delivery_state: None,
                outbound_timing: None,
                created_unix_ms: 1,
                updated_unix_ms: 1,
            }],
            dropped_turn_count: 2,
            truncated: true,
            blocker: String::new(),
            note: String::new(),
            runtime_claim: "test".to_string(),
        }
    }

    #[test]
    fn markdown_export_contains_source_translation_and_truncation_warning() {
        let body = format_markdown(&snapshot());
        assert!(body.contains("See you tomorrow."));
        assert!(body.contains("Sampai jumpa besok."));
        assert!(body.contains("2 committed turns were dropped"));
    }

    #[test]
    fn text_export_contains_source_translation_and_truncation_warning() {
        let body = format_text(&snapshot());
        assert!(body.contains("[THEM] English -> Indonesian"));
        assert!(body.contains("Source: See you tomorrow."));
        assert!(body.contains("Translation: Sampai jumpa besok."));
        assert!(body.contains("WARNING: 2 earlier committed turns"));
    }
}
