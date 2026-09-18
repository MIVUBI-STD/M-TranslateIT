use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_RUNTIME_NOTE_CHARS: usize = 360;
const MAX_RUNTIME_STATE_CHARS: usize = 120;
const APPLICATION_MEETING_OWNER_ID: &str = "translateit_application_meeting";
const DIRECT_LIVE_CAPTURE_OWNER_ID: &str = "translateit_rust_live_capture";

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSessionSnapshot {
    pub started_unix_ms: u128,
    pub owner_id: String,
    pub session_id: String,
    pub generation: u64,
    pub authority_active: bool,
    pub phase: String,
    pub live_capture_stream_active: bool,
    pub safe_to_stop: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSessionStateReport {
    pub has_active_session: bool,
    pub snapshot: Option<RuntimeSessionSnapshot>,
    pub active_age_ms: Option<u128>,
    pub ready_for_stop: bool,
    pub blocker: String,
    pub note: String,
}

static RUNTIME_SESSION_STATE: OnceLock<Mutex<Option<RuntimeSessionSnapshot>>> = OnceLock::new();
static RUNTIME_AUTHORITY_GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn begin_application_meeting_session() -> RuntimeSessionStateReport {
    begin_runtime_session(
        APPLICATION_MEETING_OWNER_ID,
        "meeting",
        "starting",
        "Application-level Meeting session authority was created. Required resources may now be opened transactionally.",
    )
}

pub fn commit_application_meeting_session_live(
    generation: u64,
    live_capture_stream_active: bool,
    note: &str,
) -> RuntimeSessionStateReport {
    let store = RUNTIME_SESSION_STATE.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = store.lock() else {
        return state_unavailable_report(
            "Meeting session could not commit Live because runtime session state is unavailable.",
        );
    };
    let Some(snapshot) = guard.as_mut() else {
        return blocked_session_report(
            "runtime_session:no_active_session",
            "Meeting session could not commit Live because no active session exists.",
        );
    };
    let current_generation = RUNTIME_AUTHORITY_GENERATION.load(Ordering::Acquire);
    if snapshot.generation != generation
        || !snapshot.authority_active
        || current_generation != generation
    {
        return RuntimeSessionStateReport {
            has_active_session: true,
            snapshot: Some(snapshot.clone()),
            active_age_ms: Some(current_unix_ms().saturating_sub(snapshot.started_unix_ms)),
            ready_for_stop: snapshot.safe_to_stop,
            blocker: "runtime_session:generation_not_authoritative".to_string(),
            note: "Meeting session Live commit was rejected because its generation no longer owns output authority.".to_string(),
        };
    }

    snapshot.phase = "live".to_string();
    snapshot.live_capture_stream_active = live_capture_stream_active;
    snapshot.note = compact_runtime_text(
        note,
        MAX_RUNTIME_NOTE_CHARS,
        "Meeting session committed Live.",
    );
    build_session_state_report(Some(snapshot.clone()))
}

pub fn revoke_runtime_session_authority(generation: u64, note: &str) -> RuntimeSessionStateReport {
    let store = RUNTIME_SESSION_STATE.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = store.lock() else {
        invalidate_runtime_generation();
        return state_unavailable_report(
            "Runtime session authority could not be verified during revoke. Output generation authority was invalidated fail-closed.",
        );
    };
    let Some(snapshot) = guard.as_mut() else {
        return build_session_state_report(None);
    };
    if snapshot.generation != generation {
        return RuntimeSessionStateReport {
            has_active_session: true,
            snapshot: Some(snapshot.clone()),
            active_age_ms: Some(current_unix_ms().saturating_sub(snapshot.started_unix_ms)),
            ready_for_stop: snapshot.safe_to_stop,
            blocker: "runtime_session:generation_mismatch".to_string(),
            note: "Runtime session authority was not changed because the requested generation is stale.".to_string(),
        };
    }

    invalidate_runtime_generation();
    snapshot.authority_active = false;
    snapshot.phase = "stopping".to_string();
    snapshot.live_capture_stream_active = false;
    snapshot.note = compact_runtime_text(
        note,
        MAX_RUNTIME_NOTE_CHARS,
        "Runtime session authority revoked before cleanup.",
    );
    build_session_state_report(Some(snapshot.clone()))
}

pub fn mark_runtime_session_cleanup_incomplete(
    generation: u64,
    live_capture_stream_active: bool,
    note: &str,
) -> RuntimeSessionStateReport {
    let store = RUNTIME_SESSION_STATE.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = store.lock() else {
        invalidate_runtime_generation();
        return state_unavailable_report(
            "Runtime cleanup could not be recorded because session state is unavailable. Output generation authority remains invalidated fail-closed.",
        );
    };
    let Some(snapshot) = guard.as_mut() else {
        return blocked_session_report(
            "runtime_session:no_active_session",
            "Runtime cleanup could not be marked incomplete because no active session exists.",
        );
    };
    if snapshot.generation != generation {
        return RuntimeSessionStateReport {
            has_active_session: true,
            snapshot: Some(snapshot.clone()),
            active_age_ms: Some(current_unix_ms().saturating_sub(snapshot.started_unix_ms)),
            ready_for_stop: snapshot.safe_to_stop,
            blocker: "runtime_session:generation_mismatch".to_string(),
            note: "Cleanup state was not changed because the requested generation is stale."
                .to_string(),
        };
    }

    invalidate_runtime_generation();
    snapshot.authority_active = false;
    snapshot.phase = "cleanup_incomplete".to_string();
    snapshot.live_capture_stream_active = live_capture_stream_active;
    snapshot.safe_to_stop = true;
    snapshot.note = compact_runtime_text(
        note,
        MAX_RUNTIME_NOTE_CHARS,
        "Runtime output authority is revoked, but one or more owned resources still need cleanup.",
    );
    build_session_state_report(Some(snapshot.clone()))
}

pub fn runtime_generation_is_authoritative(generation: u64) -> bool {
    if RUNTIME_AUTHORITY_GENERATION.load(Ordering::Acquire) != generation {
        return false;
    }
    let store = RUNTIME_SESSION_STATE.get_or_init(|| Mutex::new(None));
    store
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().cloned())
        .map(|snapshot| snapshot.generation == generation && snapshot.authority_active)
        .unwrap_or(false)
}

pub fn begin_direct_live_capture_session() -> RuntimeSessionStateReport {
    begin_runtime_session(
        DIRECT_LIVE_CAPTURE_OWNER_ID,
        "live_capture",
        "live_capture_only",
        "Direct live-capture session authority was created for microphone test ownership only.",
    )
}

pub fn latest_runtime_session_state() -> RuntimeSessionStateReport {
    let store = RUNTIME_SESSION_STATE.get_or_init(|| Mutex::new(None));
    match store.lock() {
        Ok(guard) => build_session_state_report(guard.as_ref().cloned()),
        Err(_) => state_unavailable_report(
            "Runtime session ownership could not be read. Treat runtime resources as potentially owned until the state becomes verifiable again.",
        ),
    }
}

#[cfg(test)]
pub fn clear_runtime_session_state() -> RuntimeSessionStateReport {
    invalidate_runtime_generation();
    let store = RUNTIME_SESSION_STATE.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = store.lock() else {
        return state_unavailable_report(
            "Runtime generation authority was invalidated, but test session storage could not be verified as cleared.",
        );
    };
    *guard = None;
    RuntimeSessionStateReport {
        has_active_session: false,
        snapshot: None,
        active_age_ms: None,
        ready_for_stop: false,
        blocker: "runtime_session:cleared".to_string(),
        note: "Test runtime session state was cleared and prior generation authority is invalid."
            .to_string(),
    }
}

pub fn clear_runtime_session_if_generation(generation: u64) -> RuntimeSessionStateReport {
    let store = RUNTIME_SESSION_STATE.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = store.lock() else {
        invalidate_runtime_generation();
        return state_unavailable_report(
            "Runtime generation authority was invalidated, but matching session cleanup could not be verified because session state is unavailable.",
        );
    };

    let Some(snapshot) = guard.as_ref() else {
        return build_session_state_report(None);
    };
    if snapshot.generation != generation {
        return RuntimeSessionStateReport {
            has_active_session: true,
            snapshot: Some(snapshot.clone()),
            active_age_ms: Some(current_unix_ms().saturating_sub(snapshot.started_unix_ms)),
            ready_for_stop: snapshot.safe_to_stop,
            blocker: "runtime_session:generation_mismatch".to_string(),
            note:
                "Runtime session was not cleared because the requested cleanup generation is stale."
                    .to_string(),
        };
    }

    invalidate_runtime_generation();
    *guard = None;
    RuntimeSessionStateReport {
        has_active_session: false,
        snapshot: None,
        active_age_ms: None,
        ready_for_stop: false,
        blocker: "runtime_session:cleared".to_string(),
        note: "The matching runtime session was cleared and prior generation authority is invalid."
            .to_string(),
    }
}

fn begin_runtime_session(
    owner_id: &str,
    session_prefix: &str,
    phase: &str,
    note: &str,
) -> RuntimeSessionStateReport {
    let store = RUNTIME_SESSION_STATE.get_or_init(|| Mutex::new(None));
    let Ok(mut guard) = store.lock() else {
        return state_unavailable_report(
            "Runtime session ownership could not be claimed because session state is unavailable.",
        );
    };

    if let Some(existing) = guard.as_ref() {
        return existing_session_report(existing);
    }

    let now = current_unix_ms();
    let generation = next_runtime_generation();
    let snapshot = RuntimeSessionSnapshot {
        started_unix_ms: now,
        owner_id: owner_id.to_string(),
        session_id: format!("{session_prefix}_{now}_{generation}"),
        generation,
        authority_active: true,
        phase: phase.to_string(),
        live_capture_stream_active: false,
        safe_to_stop: true,
        note: compact_runtime_text(
            note,
            MAX_RUNTIME_NOTE_CHARS,
            "Runtime session authority created.",
        ),
    };
    *guard = Some(snapshot.clone());
    build_session_state_report(Some(snapshot))
}

fn existing_session_report(existing: &RuntimeSessionSnapshot) -> RuntimeSessionStateReport {
    RuntimeSessionStateReport {
        has_active_session: true,
        snapshot: Some(existing.clone()),
        active_age_ms: Some(current_unix_ms().saturating_sub(existing.started_unix_ms)),
        ready_for_stop: existing.safe_to_stop,
        blocker: "runtime_session:already_active".to_string(),
        note: "A runtime session already owns resources. The competing claim was rejected without changing the existing owner."
            .to_string(),
    }
}

fn state_unavailable_report(note: &str) -> RuntimeSessionStateReport {
    RuntimeSessionStateReport {
        has_active_session: true,
        snapshot: None,
        active_age_ms: None,
        ready_for_stop: false,
        blocker: "runtime_session:state_lock_failed".to_string(),
        note: note.to_string(),
    }
}

fn build_session_state_report(
    snapshot: Option<RuntimeSessionSnapshot>,
) -> RuntimeSessionStateReport {
    match snapshot {
        Some(snapshot) => {
            let active_age_ms = current_unix_ms().saturating_sub(snapshot.started_unix_ms);
            let authority_active = snapshot.authority_active;
            RuntimeSessionStateReport {
                has_active_session: true,
                snapshot: Some(snapshot.clone()),
                active_age_ms: Some(active_age_ms),
                ready_for_stop: snapshot.safe_to_stop,
                blocker: if snapshot.phase == "cleanup_incomplete" {
                    "runtime_session:cleanup_incomplete".to_string()
                } else if authority_active {
                    String::new()
                } else {
                    "runtime_session:authority_revoked".to_string()
                },
                note: format!(
                    "Runtime session is in {} phase with generation {} (authority_active={authority_active}). age_ms={active_age_ms}.",
                    compact_runtime_text(&snapshot.phase, MAX_RUNTIME_STATE_CHARS, "unknown"),
                    snapshot.generation,
                ),
            }
        }
        None => RuntimeSessionStateReport {
            has_active_session: false,
            snapshot: None,
            active_age_ms: None,
            ready_for_stop: false,
            blocker: "runtime_session:no_active_session".to_string(),
            note: "No runtime session has been started yet.".to_string(),
        },
    }
}

fn blocked_session_report(blocker: &str, note: &str) -> RuntimeSessionStateReport {
    RuntimeSessionStateReport {
        has_active_session: false,
        snapshot: None,
        active_age_ms: None,
        ready_for_stop: false,
        blocker: blocker.to_string(),
        note: note.to_string(),
    }
}

fn next_runtime_generation() -> u64 {
    RUNTIME_AUTHORITY_GENERATION.fetch_add(1, Ordering::AcqRel) + 1
}

fn invalidate_runtime_generation() -> u64 {
    RUNTIME_AUTHORITY_GENERATION.fetch_add(1, Ordering::AcqRel) + 1
}

fn is_unsafe_runtime_state_character(character: char) -> bool {
    character == '\0'
        || ('\u{0001}'..='\u{0008}').contains(&character)
        || ('\u{000b}'..='\u{001f}').contains(&character)
        || character == '\u{007f}'
        || ('\u{202a}'..='\u{202e}').contains(&character)
        || ('\u{2066}'..='\u{2069}').contains(&character)
}

fn compact_runtime_text(value: &str, max_chars: usize, fallback: &str) -> String {
    let clean = value
        .trim()
        .chars()
        .filter(|character| !is_unsafe_runtime_state_character(*character))
        .take(max_chars)
        .collect::<String>();
    if clean.is_empty() {
        fallback.to_string()
    } else {
        clean
    }
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_SERIAL: Mutex<()> = Mutex::new(());

    fn reset_test_state() {
        let store = RUNTIME_SESSION_STATE.get_or_init(|| Mutex::new(None));
        if let Ok(mut guard) = store.lock() {
            *guard = None;
        }
        RUNTIME_AUTHORITY_GENERATION.store(0, Ordering::Release);
    }

    #[test]
    fn competing_runtime_claims_never_overwrite_the_existing_owner() {
        let _serial = TEST_SERIAL.lock().expect("runtime-state test lock");
        reset_test_state();

        let meeting = begin_application_meeting_session();
        assert!(meeting.blocker.is_empty());
        let mic_after_meeting = begin_direct_live_capture_session();
        assert_eq!(mic_after_meeting.blocker, "runtime_session:already_active");
        assert_eq!(
            mic_after_meeting
                .snapshot
                .as_ref()
                .map(|value| value.owner_id.as_str()),
            Some(APPLICATION_MEETING_OWNER_ID),
        );

        reset_test_state();
        let mic = begin_direct_live_capture_session();
        assert!(mic.blocker.is_empty());
        let meeting_after_mic = begin_application_meeting_session();
        assert_eq!(meeting_after_mic.blocker, "runtime_session:already_active");
        assert_eq!(
            meeting_after_mic
                .snapshot
                .as_ref()
                .map(|value| value.owner_id.as_str()),
            Some(DIRECT_LIVE_CAPTURE_OWNER_ID),
        );

        reset_test_state();
    }

    #[test]
    fn unavailable_runtime_state_is_fail_closed_not_idle() {
        let report = state_unavailable_report("test");
        assert!(report.has_active_session);
        assert!(report.snapshot.is_none());
        assert!(!report.ready_for_stop);
        assert_eq!(report.blocker, "runtime_session:state_lock_failed");
    }

    #[test]
    fn duplicate_application_meeting_claim_preserves_generation_and_session() {
        let _serial = TEST_SERIAL.lock().expect("runtime-state test lock");
        reset_test_state();

        let first = begin_application_meeting_session();
        let first_snapshot = first.snapshot.as_ref().expect("first meeting claim");
        let generation = first_snapshot.generation;
        let session_id = first_snapshot.session_id.clone();
        assert!(runtime_generation_is_authoritative(generation));

        let duplicate = begin_application_meeting_session();
        let duplicate_snapshot = duplicate.snapshot.as_ref().expect("duplicate meeting claim");
        assert_eq!(duplicate.blocker, "runtime_session:already_active");
        assert_eq!(duplicate_snapshot.generation, generation);
        assert_eq!(duplicate_snapshot.session_id, session_id);
        assert_eq!(duplicate_snapshot.owner_id, APPLICATION_MEETING_OWNER_ID);
        assert_eq!(duplicate_snapshot.phase, "starting");
        assert!(duplicate_snapshot.authority_active);
        assert!(runtime_generation_is_authoritative(generation));

        reset_test_state();
    }

    #[test]
    fn stale_live_commit_does_not_promote_current_session() {
        let _serial = TEST_SERIAL.lock().expect("runtime-state test lock");
        reset_test_state();

        let meeting = begin_application_meeting_session();
        let generation = meeting.snapshot.as_ref().expect("meeting claim").generation;
        let stale = commit_application_meeting_session_live(
            generation.saturating_add(1),
            true,
            "stale commit must not promote",
        );
        let stale_snapshot = stale.snapshot.as_ref().expect("retained meeting snapshot");
        assert_eq!(stale.blocker, "runtime_session:generation_not_authoritative");
        assert_eq!(stale_snapshot.generation, generation);
        assert_eq!(stale_snapshot.phase, "starting");
        assert!(stale_snapshot.authority_active);
        assert!(!stale_snapshot.live_capture_stream_active);
        assert!(runtime_generation_is_authoritative(generation));

        let committed = commit_application_meeting_session_live(
            generation,
            true,
            "authoritative commit may promote",
        );
        let committed_snapshot = committed.snapshot.as_ref().expect("live meeting snapshot");
        assert!(committed.blocker.is_empty());
        assert_eq!(committed_snapshot.phase, "live");
        assert!(committed_snapshot.live_capture_stream_active);
        assert!(committed_snapshot.authority_active);

        reset_test_state();
    }

    #[test]
    fn revoke_invalidates_generation_before_cleanup() {
        let _serial = TEST_SERIAL.lock().expect("runtime-state test lock");
        reset_test_state();

        let meeting = begin_application_meeting_session();
        let generation = meeting.snapshot.as_ref().expect("meeting claim").generation;
        assert!(runtime_generation_is_authoritative(generation));

        let revoked = revoke_runtime_session_authority(generation, "stop accepted");
        let snapshot = revoked.snapshot.as_ref().expect("revoked meeting snapshot");
        assert_eq!(revoked.blocker, "runtime_session:authority_revoked");
        assert_eq!(snapshot.generation, generation);
        assert_eq!(snapshot.phase, "stopping");
        assert!(!snapshot.authority_active);
        assert!(!snapshot.live_capture_stream_active);
        assert!(!runtime_generation_is_authoritative(generation));

        reset_test_state();
    }

    #[test]
    fn stale_cleanup_generation_cannot_clear_a_newer_owner() {
        let _serial = TEST_SERIAL.lock().expect("runtime-state test lock");
        reset_test_state();

        let meeting = begin_application_meeting_session();
        let meeting_generation = meeting.snapshot.as_ref().expect("meeting claim").generation;
        let cleared = clear_runtime_session_if_generation(meeting_generation);
        assert_eq!(cleared.blocker, "runtime_session:cleared");

        let mic_test = begin_direct_live_capture_session();
        let mic_generation = mic_test.snapshot.as_ref().expect("mic claim").generation;
        assert_ne!(meeting_generation, mic_generation);

        let stale_clear = clear_runtime_session_if_generation(meeting_generation);
        assert_eq!(stale_clear.blocker, "runtime_session:generation_mismatch");
        assert_eq!(
            stale_clear.snapshot.as_ref().map(|value| value.generation),
            Some(mic_generation),
        );
        assert_eq!(
            stale_clear
                .snapshot
                .as_ref()
                .map(|value| value.owner_id.as_str()),
            Some(DIRECT_LIVE_CAPTURE_OWNER_ID),
        );

        let final_clear = clear_runtime_session_if_generation(mic_generation);
        assert_eq!(final_clear.blocker, "runtime_session:cleared");
        reset_test_state();
    }

    #[test]
    fn stale_revoke_cannot_cancel_a_newer_owner() {
        let _serial = TEST_SERIAL.lock().expect("runtime-state test lock");
        reset_test_state();

        let meeting = begin_application_meeting_session();
        let meeting_generation = meeting.snapshot.as_ref().expect("meeting claim").generation;
        assert_eq!(
            clear_runtime_session_if_generation(meeting_generation).blocker,
            "runtime_session:cleared"
        );

        let mic = begin_direct_live_capture_session();
        let mic_generation = mic.snapshot.as_ref().expect("mic claim").generation;
        assert!(runtime_generation_is_authoritative(mic_generation));

        let stale_revoke = revoke_runtime_session_authority(meeting_generation, "stale stop");
        let snapshot = stale_revoke.snapshot.as_ref().expect("newer owner retained");
        assert_eq!(stale_revoke.blocker, "runtime_session:generation_mismatch");
        assert_eq!(snapshot.generation, mic_generation);
        assert_eq!(snapshot.owner_id, DIRECT_LIVE_CAPTURE_OWNER_ID);
        assert!(snapshot.authority_active);
        assert!(runtime_generation_is_authoritative(mic_generation));

        assert_eq!(
            clear_runtime_session_if_generation(mic_generation).blocker,
            "runtime_session:cleared"
        );
        reset_test_state();
    }

    #[test]
    fn cleanup_incomplete_retains_owner_until_successful_retry_clear() {
        let _serial = TEST_SERIAL.lock().expect("runtime-state test lock");
        reset_test_state();

        let meeting = begin_application_meeting_session();
        let generation = meeting.snapshot.as_ref().expect("meeting claim").generation;
        let revoked =
            revoke_runtime_session_authority(generation, "test revoke before incomplete cleanup");
        assert!(
            !revoked
                .snapshot
                .as_ref()
                .expect("revoked snapshot")
                .authority_active
        );

        let incomplete = mark_runtime_session_cleanup_incomplete(
            generation,
            true,
            "microphone cleanup still needs attention",
        );
        let snapshot = incomplete
            .snapshot
            .as_ref()
            .expect("retained cleanup owner");
        assert_eq!(snapshot.phase, "cleanup_incomplete");
        assert!(snapshot.live_capture_stream_active);
        assert!(!snapshot.authority_active);
        assert_eq!(incomplete.blocker, "runtime_session:cleanup_incomplete");

        let competing = begin_direct_live_capture_session();
        assert_eq!(competing.blocker, "runtime_session:already_active");
        assert_eq!(
            competing
                .snapshot
                .as_ref()
                .map(|value| value.owner_id.as_str()),
            Some(APPLICATION_MEETING_OWNER_ID),
        );

        let stale_retry = clear_runtime_session_if_generation(generation.saturating_add(1));
        assert_eq!(stale_retry.blocker, "runtime_session:generation_mismatch");
        assert_eq!(
            stale_retry.snapshot.as_ref().map(|value| value.generation),
            Some(generation)
        );
        assert_eq!(
            stale_retry.snapshot.as_ref().map(|value| value.phase.as_str()),
            Some("cleanup_incomplete")
        );

        let cleared = clear_runtime_session_if_generation(generation);
        assert!(!cleared.has_active_session);
        assert!(cleared.snapshot.is_none());
        assert_eq!(cleared.blocker, "runtime_session:cleared");
        reset_test_state();
    }
}
