use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::engine::paths::ProjectPaths;

use super::incident_log::{get_recent_runtime_incidents, record_runtime_incident};
use super::meeting_session::{
    deferred_incoming_health_counts, get_meeting_committed_turns, get_meeting_session_status,
};

const TEMP_FILE_WARNING_THRESHOLD: usize = 12;

#[derive(Debug, Clone, Serialize)]
pub struct LongSessionHealthStatus {
    pub state: String,
    pub healthy: bool,
    pub session_age_ms: u128,
    pub outbound_overflow_dropped: u64,
    pub outbound_evicted_pending: u64,
    pub deferred_incoming_depth: usize,
    pub deferred_incoming_dropped_overflow: u64,
    pub deferred_incoming_dropped_stale: u64,
    pub transcript_dropped_turns: u64,
    pub transcript_truncated: bool,
    pub meeting_temp_file_count: usize,
    pub incident_count: usize,
    pub warning_count: usize,
    pub note: String,
    pub updated_unix_ms: u128,
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn matching_file_count(directory: &Path, predicate: impl Fn(&str) -> bool) -> usize {
    fs::read_dir(directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().map(|kind| kind.is_file()).unwrap_or(false))
        .filter(|entry| predicate(&entry.file_name().to_string_lossy()))
        .count()
}

fn meeting_temp_file_count(paths: &ProjectPaths) -> usize {
    let cache = PathBuf::from(&paths.user_cache_dir);
    matching_file_count(&cache.join("audio_segments"), |name| {
        name.starts_with("final_") && (name.ends_with(".wav") || name.ends_with(".wav.tmp"))
    })
    .saturating_add(matching_file_count(&cache.join("meeting_tts"), |name| {
        name.ends_with(".wav") || name.ends_with(".wav.tmp")
    }))
}

#[tauri::command]
pub fn get_long_session_health_status() -> LongSessionHealthStatus {
    let meeting = get_meeting_session_status();
    let transcript = get_meeting_committed_turns();
    let (deferred_depth, deferred_overflow, deferred_stale) =
        deferred_incoming_health_counts();
    let temp_count = meeting_temp_file_count(&ProjectPaths::discover());
    let incidents = get_recent_runtime_incidents();

    let overflow = meeting.outbound.overflow_dropped_utterance_count;
    let evicted = meeting.outbound.evicted_pending_utterance_count;
    let mut warnings = 0usize;
    warnings += usize::from(overflow > 0);
    warnings += usize::from(evicted > 0);
    warnings += usize::from(deferred_overflow > 0);
    warnings += usize::from(deferred_stale > 0);
    warnings += usize::from(transcript.dropped_turn_count > 0);
    warnings += usize::from(temp_count > TEMP_FILE_WARNING_THRESHOLD);

    let healthy = warnings == 0;
    let state = if !meeting.has_session {
        "idle"
    } else if healthy {
        "healthy"
    } else {
        "attention"
    };
    let note = if !meeting.has_session {
        "No Meeting session is active. Long-session pressure counters remain bounded and are available for Diagnostics."
    } else if healthy {
        "No bounded queue, transcript, or Meeting temp-file pressure has been observed in the current session."
    } else {
        "One or more bounded long-session pressure counters are non-zero. This status is diagnostic only; TranslateIT does not silently clear or replay work."
    };

    let result = LongSessionHealthStatus {
        state: state.to_string(),
        healthy,
        session_age_ms: meeting.active_age_ms.unwrap_or(0),
        outbound_overflow_dropped: overflow,
        outbound_evicted_pending: evicted,
        deferred_incoming_depth: deferred_depth,
        deferred_incoming_dropped_overflow: deferred_overflow,
        deferred_incoming_dropped_stale: deferred_stale,
        transcript_dropped_turns: transcript.dropped_turn_count,
        transcript_truncated: transcript.truncated,
        meeting_temp_file_count: temp_count,
        incident_count: incidents.count,
        warning_count: warnings,
        note: note.to_string(),
        updated_unix_ms: unix_ms(),
    };

    if meeting.has_session && !result.healthy {
        let _ = record_runtime_incident(
            "long_session_health",
            "meeting_runtime",
            "long_session_health:pressure_detected",
            &result.note,
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::TEMP_FILE_WARNING_THRESHOLD;

    #[test]
    fn temp_file_warning_threshold_is_bounded_but_not_single_file_sensitive() {
        assert!(TEMP_FILE_WARNING_THRESHOLD >= 4);
        assert!(TEMP_FILE_WARNING_THRESHOLD <= 32);
    }
}
