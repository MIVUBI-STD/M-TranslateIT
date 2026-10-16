use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use crate::engine::runtime_events::emit_meeting_runtime_event;
use crate::engine::runtime_state::latest_runtime_session_state;

use super::super::helper_bridge_runtime::unix_ms;
use super::transcript_archive;
use super::{
    MeetingCommittedTurn, MeetingCommittedTurnsSnapshot, MeetingOutboundTiming,
    APPLICATION_MEETING_OWNER_ID,
};

const MAX_LIVE_COMMITTED_TURNS: usize = 240;
// The UI stays small, but manual export may retain a much longer session.
// Both limits are hard bounds; excess is explicitly counted as lost content.
const MAX_SESSION_COMMITTED_TURNS: usize = 4_096;
const MAX_SESSION_TRANSCRIPT_BYTES: usize = 8 * 1024 * 1024;

fn turn_text_bytes(turn: &MeetingCommittedTurn) -> usize {
    turn.source_text.len().saturating_add(turn.translated_text.len())
}

struct MeetingCommittedTurnStore {
    session_id: String,
    dropped_turn_count: u64,
    retained_text_bytes: usize,
    // Compact replay fence independent of bounded transcript retention.
    retired_sequence: u64,
    turns: VecDeque<MeetingCommittedTurn>,
}

impl MeetingCommittedTurnStore {
    fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            dropped_turn_count: 0,
            retained_text_bytes: 0,
            retired_sequence: 0,
            turns: VecDeque::new(),
        }
    }

    fn enforce_session_limits(&mut self) {
        while self.turns.len() > MAX_SESSION_COMMITTED_TURNS
            || self.retained_text_bytes > MAX_SESSION_TRANSCRIPT_BYTES
        {
            let Some(oldest) = self.turns.pop_front() else {
                self.retained_text_bytes = 0;
                break;
            };
            self.retained_text_bytes =
                self.retained_text_bytes.saturating_sub(turn_text_bytes(&oldest));
            self.retired_sequence = self.retired_sequence.max(oldest.sequence);
            self.dropped_turn_count = self.dropped_turn_count.saturating_add(1);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn commit_at(
        &mut self,
        session_id: &str,
        sequence: u64,
        generation: Option<u64>,
        utterance_id: u64,
        lane: &str,
        source_language: &str,
        target_language: &str,
        source_text: &str,
        translated_text: &str,
        delivery_state: Option<&str>,
        outbound_timing: Option<MeetingOutboundTiming>,
        now_unix_ms: u128,
    ) -> Option<MeetingCommittedTurn> {
        if self.session_id != session_id
            || sequence == 0
            || !matches!(lane, "you" | "incoming")
            || !matches!(source_language, "id" | "en")
            || !matches!(target_language, "id" | "en")
            || source_language == target_language
        {
            return None;
        }
        if source_text.trim().is_empty() {
            return None;
        }
        if lane == "you"
            && (generation.is_none() || delivery_state.is_none() || outbound_timing.is_none())
        {
            return None;
        }
        if lane == "incoming"
            && (generation.is_some() || delivery_state.is_some() || outbound_timing.is_some())
        {
            return None;
        }
        // Only a newly inserted source can start MT/TTS. A duplicate sequence
        // or utterance identity must not reopen inference; evicted history is
        // protected by a compact monotonic sequence fence.
        if sequence <= self.retired_sequence
            || self.turns.iter().any(|turn| {
                turn.sequence == sequence
                    || (turn.lane == lane
                        && turn.generation == generation
                        && turn.utterance_id == utterance_id)
            })
        {
            return None;
        }

        let turn = MeetingCommittedTurn {
            session_id: session_id.to_string(),
            sequence,
            generation,
            utterance_id,
            lane: lane.to_string(),
            source_language: source_language.to_string(),
            target_language: target_language.to_string(),
            source_text: source_text.to_string(),
            translated_text: translated_text.to_string(),
            translation_state: if translated_text.trim().is_empty() {
                "pending".to_string()
            } else {
                "translated".to_string()
            },
            delivery_state: delivery_state.map(str::to_string),
            outbound_timing,
            created_unix_ms: now_unix_ms,
            updated_unix_ms: now_unix_ms,
        };
        let insert_at = self
            .turns
            .iter()
            .position(|existing| existing.sequence > sequence)
            .unwrap_or(self.turns.len());
        self.retained_text_bytes =
            self.retained_text_bytes.saturating_add(turn_text_bytes(&turn));
        // The archive owns an accepted copy before bounded Live memory can
        // evict it, including a single unusually long recognized utterance.
        let archived = turn.clone();
        self.turns.insert(insert_at, turn);
        self.enforce_session_limits();
        Some(archived)
    }

    fn finish_translation_at(
        &mut self,
        session_id: &str,
        sequence: u64,
        generation: Option<u64>,
        utterance_id: u64,
        lane: &str,
        translated: Option<&str>,
        now_unix_ms: u128,
    ) -> Option<MeetingCommittedTurn> {
        if self.session_id != session_id || sequence == 0 || !matches!(lane, "you" | "incoming") {
            return None;
        }
        let Some(turn) = self.turns.iter_mut().find(|turn| {
            turn.session_id == session_id && turn.sequence == sequence
                && turn.generation == generation && turn.utterance_id == utterance_id
                && turn.lane == lane
        }) else {
            return None;
        };
        // Once a turn is complete, failed, or interrupted, stale results cannot
        // rewrite its recognized text, translation or playback state.
        if turn.translation_state != "pending"
            || terminal_delivery_state(turn.delivery_state.as_deref())
        {
            return None;
        }
        let previous_bytes = turn.translated_text.len();
        if let Some(text) = translated.filter(|text| !text.trim().is_empty()) {
            turn.translated_text = text.trim().to_string();
            turn.translation_state = "translated".to_string();
            if lane == "you" {
                turn.delivery_state = Some("preparing_voice".to_string());
            }
        } else {
            turn.translated_text.clear();
            turn.translation_state = "failed".to_string();
            if lane == "you" {
                turn.delivery_state = Some("output_failed".to_string());
            }
        }
        turn.updated_unix_ms = now_unix_ms;
        // Retention may evict this turn when its translation expands the
        // text budget. Return the verified final version to the archive first.
        let archived = turn.clone();
        self.retained_text_bytes = self.retained_text_bytes
            .saturating_sub(previous_bytes)
            .saturating_add(turn.translated_text.len());
        self.enforce_session_limits();
        Some(archived)
    }

    fn update_delivery_state_at(
        &mut self,
        session_id: &str,
        generation: u64,
        utterance_id: u64,
        delivery_state: &str,
        now_unix_ms: u128,
    ) -> bool {
        if self.session_id != session_id
            || !matches!(
                delivery_state,
                "translating"
                    | "preparing_voice"
                    | "queued"
                    | "speaking"
                    | "output_complete"
                    | "output_failed"
                    | "interrupted"
            )
        {
            return false;
        }
        let Some(turn) = self.turns.iter_mut().find(|turn| {
            turn.session_id == session_id
                && turn.generation == Some(generation)
                && turn.utterance_id == utterance_id
                && turn.lane == "you"
        }) else {
            return false;
        };

        if terminal_delivery_state(turn.delivery_state.as_deref()) {
            return turn.delivery_state.as_deref() == Some(delivery_state);
        }
        // A stale enqueue cannot send a translated or playing unit backwards.
        if delivery_state == "queued"
            && (turn.translation_state != "translated"
                || turn.delivery_state.as_deref() != Some("preparing_voice"))
        {
            return false;
        }
        turn.delivery_state = Some(delivery_state.to_string());
        turn.updated_unix_ms = now_unix_ms;
        true
    }

    fn begin_playback_at(
        &mut self,
        session_id: &str,
        generation: u64,
        sequence: u64,
        utterance_id: u64,
        now_unix_ms: u128,
    ) -> bool {
        if self.session_id != session_id || sequence == 0 {
            return false;
        }
        let Some(turn) = self.turns.iter_mut().find(|turn| {
            turn.sequence == sequence
                && turn.generation == Some(generation)
                && turn.utterance_id == utterance_id
                && turn.lane == "you"
        }) else {
            return false;
        };
        if turn.translation_state != "translated"
            || turn.delivery_state.as_deref() != Some("queued")
        {
            return false;
        }
        // Claim queued -> speaking exactly once before native audio is attempted.
        turn.delivery_state = Some("speaking".to_string());
        turn.updated_unix_ms = now_unix_ms;
        true
    }

    fn update_outbound_timing_at(
        &mut self,
        session_id: &str,
        generation: u64,
        utterance_id: u64,
        timing: &MeetingOutboundTiming,
        now_unix_ms: u128,
    ) -> bool {
        if self.session_id != session_id {
            return false;
        }
        let Some(turn) = self.turns.iter_mut().find(|turn| {
            turn.session_id == session_id
                && turn.generation == Some(generation)
                && turn.utterance_id == utterance_id
                && turn.lane == "you"
        }) else {
            return false;
        };
        turn.outbound_timing = Some(timing.clone());
        turn.updated_unix_ms = now_unix_ms;
        true
    }

    fn interrupt_generation_at(
        &mut self,
        generation: u64,
        now_unix_ms: u128,
    ) -> Vec<MeetingCommittedTurn> {
        let mut changed = Vec::new();
        for turn in &mut self.turns {
            if turn.generation == Some(generation)
                && !terminal_delivery_state(turn.delivery_state.as_deref())
            {
                turn.delivery_state = Some("interrupted".to_string());
                if turn.translation_state == "pending" {
                    turn.translation_state = "failed".to_string();
                }
                turn.updated_unix_ms = now_unix_ms;
                // Only actual changed entries need a durable update.
                changed.push(turn.clone());
            }
        }
        changed
    }

    // On-demand aggregate over already bounded committed turns. Only
    // successful outbound playback with a measured first-audio time counts.
    fn outbound_latency_distribution(&self) -> Option<(usize, u64, u64)> {
        let mut samples: Vec<u64> = self.turns.iter()
            .filter(|turn| turn.lane == "you"
                && turn.delivery_state.as_deref() == Some("output_complete"))
            .filter_map(|turn| turn.outbound_timing.as_ref()
                .and_then(|timing| timing.outbound_latency_ms))
            .collect();
        if samples.is_empty() {
            return None;
        }
        samples.sort_unstable();
        let count = samples.len();
        // Nearest-rank P50/P95 over the already capped rolling turn store.
        let p50 = samples[(count * 50).div_ceil(100) - 1];
        let p95 = samples[(count * 95).div_ceil(100) - 1];
        Some((count, p50, p95))
    }

    fn snapshot_for_export(&self) -> MeetingCommittedTurnsSnapshot {
        // Stored turns are already ordered by shared sequence; no re-sort needed.
        MeetingCommittedTurnsSnapshot {
            ok: true,
            has_session: true,
            session_id: Some(self.session_id.clone()),
            turns: self.turns.iter().cloned().collect(),
            dropped_turn_count: self.dropped_turn_count,
            archive_write_errors: transcript_archive::active_write_errors(&self.session_id),
            truncated: self.dropped_turn_count > 0,
            blocker: String::new(),
            note: if self.dropped_turn_count > 0 {
                format!(
                    "The Live-memory buffer released {} earlier turns. The saved disk archive may retain them if each write succeeded.",
                    self.dropped_turn_count,
                )
            } else {
                "Complete currently retained Meeting turns for explicit manual export.".to_string()
            },
            runtime_claim: "meeting_committed_turn_snapshot_source_contract_not_rendered_runtime_proof".to_string(),
        }
    }

    fn snapshot(&self) -> MeetingCommittedTurnsSnapshot {
        // Fast path for live UI refresh: clone only the last visible turns,
        // not thousands of retained turns on every runtime event/poll.
        let mut turns: Vec<_> = self.turns.iter().rev()
            .take(MAX_LIVE_COMMITTED_TURNS).cloned().collect();
        turns.reverse();
        MeetingCommittedTurnsSnapshot {
            ok: true,
            has_session: true,
            session_id: Some(self.session_id.clone()),
            turns,
            dropped_turn_count: self.dropped_turn_count,
            archive_write_errors: transcript_archive::active_write_errors(&self.session_id),
            truncated: self.dropped_turn_count > 0,
            blocker: String::new(),
            note: if self.dropped_turn_count > 0 {
                format!("{} earlier phrases left the bounded Live memory buffer; saved disk entries may still be available.", self.dropped_turn_count)
            } else if self.turns.len() > MAX_LIVE_COMMITTED_TURNS {
                "Showing the latest 240 phrases. Earlier phrases remain available for manual export.".to_string()
            } else {
                "Live Meeting transcription reflects the latest retained recognized source and available translations.".to_string()
            },
            runtime_claim: "meeting_committed_turn_snapshot_source_contract_not_rendered_runtime_proof".to_string(),
        }
    }
}

static MEETING_COMMITTED_TURNS: OnceLock<Mutex<Option<MeetingCommittedTurnStore>>> =
    OnceLock::new();
static LAST_ENDED_MEETING_TRANSCRIPT: OnceLock<Mutex<Option<MeetingCommittedTurnsSnapshot>>> =
    OnceLock::new();

fn committed_turn_store() -> &'static Mutex<Option<MeetingCommittedTurnStore>> {
    MEETING_COMMITTED_TURNS.get_or_init(|| Mutex::new(None))
}

fn ended_transcript_store() -> &'static Mutex<Option<MeetingCommittedTurnsSnapshot>> {
    LAST_ENDED_MEETING_TRANSCRIPT.get_or_init(|| Mutex::new(None))
}

fn terminal_delivery_state(state: Option<&str>) -> bool {
    matches!(
        state,
        Some("output_complete" | "output_failed" | "interrupted")
    )
}

pub(super) fn reset_committed_turns(session_id: &str) {
    if let Ok(mut ended) = ended_transcript_store().lock() {
        *ended = None;
    }
    if let Ok(mut guard) = committed_turn_store().lock() {
        *guard = Some(MeetingCommittedTurnStore::new(session_id));
    }
}

pub(super) fn clear_committed_turns_for_session(session_id: &str) -> bool {
    // Flush the bounded background disk writer before releasing in-memory
    // transcript ownership; failures stay recoverable through Retry Stop.
    if !transcript_archive::finish_session(session_id) {
        return false;
    }
    let Ok(mut guard) = committed_turn_store().lock() else {
        return false;
    };
    if guard
        .as_ref()
        .map(|store| store.session_id == session_id)
        .unwrap_or(false)
    {
        *guard = None;
    }
    true
}

// No-session Stop may discard orphaned active turns but must preserve the
// last explicitly ended Meeting until the next Start resets it for privacy.
// Explicit archive deletion also removes the private in-memory last-session review.
pub(super) fn forget_ended_saved_session(session_id: &str) {
    if let Ok(mut ended) = ended_transcript_store().lock() {
        if ended.as_ref().and_then(|snapshot| snapshot.session_id.as_deref()) == Some(session_id) {
            *ended = None;
        }
    }
}

pub(super) fn clear_inactive_committed_turns() -> bool {
    let Ok(mut guard) = committed_turn_store().lock() else {
        return false;
    };
    *guard = None;
    true
}

pub(super) fn retain_committed_turns_for_export(session_id: &str) -> bool {
    let Ok(guard) = committed_turn_store().lock() else {
        return false;
    };
    let Some(store) = guard.as_ref() else {
        return true;
    };
    if store.session_id != session_id {
        return false;
    }
    let mut snapshot = store.snapshot_for_export();
    snapshot.has_session = false;
    // Ending a session also ends all uncompleted incoming/outgoing work. The
    // last-session review must never display "Translating..." forever.
    for turn in &mut snapshot.turns {
        if turn.translation_state == "pending" {
            turn.translation_state = "failed".to_string();
            if turn.lane == "you" {
                turn.delivery_state = Some("interrupted".to_string());
            }
        }
    }
    drop(guard);
    let Ok(mut ended) = ended_transcript_store().lock() else {
        return false;
    };
    *ended = if snapshot.turns.is_empty() { None } else { Some(snapshot) };
    true
}

pub(super) fn exportable_committed_turn_snapshot() -> Option<MeetingCommittedTurnsSnapshot> {
    // Only the active authoritative Meeting session or the last explicitly
    // stopped session may be exported. Never expose a foreign runtime owner.
    if let Some(session) = latest_runtime_session_state().snapshot {
        if session.owner_id == APPLICATION_MEETING_OWNER_ID {
            if let Ok(guard) = committed_turn_store().lock() {
                if let Some(store) = guard.as_ref() {
                    if store.session_id == session.session_id && !store.turns.is_empty() {
                        return Some(store.snapshot_for_export());
                    }
                }
            }
        }
    }
    ended_transcript_store().lock().ok().and_then(|guard| guard.clone())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn commit_meeting_turn(
    session_id: &str,
    sequence: u64,
    generation: Option<u64>,
    utterance_id: u64,
    lane: &str,
    source_language: &str,
    target_language: &str,
    source_text: &str,
    translated_text: &str,
    delivery_state: Option<&str>,
    outbound_timing: Option<MeetingOutboundTiming>,
) -> bool {
    let Some(session) = latest_runtime_session_state().snapshot else {
        return false;
    };
    if session.owner_id != APPLICATION_MEETING_OWNER_ID
        || session.session_id != session_id
        || session.phase != "live"
        || generation.is_some_and(|current| session.generation != current)
    {
        return false;
    }
    let Ok(mut guard) = committed_turn_store().lock() else {
        return false;
    };
    let Some(store) = guard.as_mut() else {
        return false;
    };
    let committed = store.commit_at(
        session_id,
        sequence,
        generation,
        utterance_id,
        lane,
        source_language,
        target_language,
        source_text,
        translated_text,
        delivery_state,
        outbound_timing,
        unix_ms(),
    );
    let accepted = committed.is_some();
    if let Some(turn) = committed {
        transcript_archive::queue_turn(turn);
    }
    drop(guard);
    if accepted {
        emit_meeting_runtime_event("turn_committed", Some(session_id), Some(sequence));
    }
    accepted
}

pub(super) fn finish_meeting_turn_translation(
    session_id: &str,
    sequence: u64,
    generation: Option<u64>,
    utterance_id: u64,
    lane: &str,
    translated: Option<&str>,
) -> bool {
    let Some(session) = latest_runtime_session_state().snapshot else {
        return false;
    };
    if session.owner_id != APPLICATION_MEETING_OWNER_ID
        || session.session_id != session_id
        || session.phase != "live"
        || generation.is_some_and(|current| session.generation != current)
    {
        return false;
    }
    let Ok(mut guard) = committed_turn_store().lock() else {
        return false;
    };
    let Some(store) = guard.as_mut() else {
        return false;
    };
    let updated = store.finish_translation_at(
        session_id, sequence, generation, utterance_id, lane, translated, unix_ms(),
    );
    let accepted = updated.is_some();
    if let Some(turn) = updated {
        transcript_archive::queue_turn(turn);
    }
    drop(guard);
    if accepted {
        emit_meeting_runtime_event("translation_state_changed", Some(session_id), Some(sequence));
    }
    accepted
}

// Sequence + source identity and queued state must all agree before audio.
pub(super) fn begin_committed_playback(
    session_id: &str,
    generation: u64,
    sequence: u64,
    utterance_id: u64,
) -> bool {
    let Some(session) = latest_runtime_session_state().snapshot else {
        return false;
    };
    if session.owner_id != APPLICATION_MEETING_OWNER_ID
        || session.session_id != session_id
        || session.phase != "live"
        || session.generation != generation
    {
        return false;
    }
    let Ok(mut guard) = committed_turn_store().lock() else {
        return false;
    };
    let Some(store) = guard.as_mut() else {
        return false;
    };
    let claimed = store.begin_playback_at(session_id, generation, sequence, utterance_id, unix_ms());
    if claimed {
        if let Some(turn) = store.turns.iter().find(|turn| turn.sequence == sequence) {
            transcript_archive::queue_turn(turn.clone());
        }
    }
    drop(guard);
    if claimed {
        emit_meeting_runtime_event("delivery_state_changed", Some(session_id), Some(sequence));
    }
    claimed
}

pub(super) fn update_committed_turn_delivery_state(
    session_id: &str,
    generation: u64,
    utterance_id: u64,
    delivery_state: &str,
) -> bool {
    let Ok(mut guard) = committed_turn_store().lock() else {
        return false;
    };
    let Some(store) = guard.as_mut() else {
        return false;
    };
    let updated = store.update_delivery_state_at(
        session_id,
        generation,
        utterance_id,
        delivery_state,
        unix_ms(),
    );
    if updated {
        if let Some(turn) = store.turns.iter().find(|turn| {
            turn.generation == Some(generation) && turn.utterance_id == utterance_id
                && turn.lane == "you"
        }) {
            // Terminal duplicate acknowledgements can be idempotent; the
            // single writer still replaces this exact entry, never appends.
            transcript_archive::queue_turn(turn.clone());
        }
    }
    drop(guard);
    if updated {
        emit_meeting_runtime_event("delivery_state_changed", Some(session_id), None);
    }
    updated
}

pub(super) fn update_committed_turn_outbound_timing(
    session_id: &str,
    generation: u64,
    utterance_id: u64,
    timing: &MeetingOutboundTiming,
) -> bool {
    let Ok(mut guard) = committed_turn_store().lock() else {
        return false;
    };
    let Some(store) = guard.as_mut() else {
        return false;
    };
    let updated = store.update_outbound_timing_at(session_id, generation, utterance_id, timing, unix_ms());
    if updated {
        if let Some(turn) = store.turns.iter().find(|turn| {
            turn.generation == Some(generation) && turn.utterance_id == utterance_id
                && turn.lane == "you"
        }) {
            transcript_archive::queue_turn(turn.clone());
        }
    }
    updated
}

pub(super) fn interrupt_committed_turns_for_generation(session_id: &str, generation: u64) {
    let Ok(mut guard) = committed_turn_store().lock() else {
        return;
    };
    let Some(store) = guard.as_mut() else {
        return;
    };
    if store.session_id != session_id {
        return;
    }
    // Stop's interruption state must reach the disk queue before the Finish
    // record. The writer preserves exactly the enqueue order of mutations.
    for turn in store.interrupt_generation_at(generation, unix_ms()) {
        transcript_archive::queue_turn(turn);
    }
}

fn empty_committed_turn_snapshot(
    has_session: bool,
    session_id: Option<String>,
    blocker: &str,
    note: &str,
) -> MeetingCommittedTurnsSnapshot {
    MeetingCommittedTurnsSnapshot {
        ok: blocker.is_empty(),
        has_session,
        session_id,
        turns: Vec::new(),
        dropped_turn_count: 0,
        archive_write_errors: 0,
        truncated: false,
        blocker: blocker.to_string(),
        note: note.to_string(),
        runtime_claim: "meeting_committed_turn_snapshot_source_contract_not_rendered_runtime_proof"
            .to_string(),
    }
}

pub(super) fn committed_turn_health_counts() -> (usize, u64, bool) {
    committed_turn_store()
        .lock()
        .ok()
        .and_then(|guard| {
            guard.as_ref().map(|store| {
                (
                    store.turns.len().min(MAX_LIVE_COMMITTED_TURNS),
                    store.dropped_turn_count,
                    store.dropped_turn_count > 0,
                )
            })
        })
        .unwrap_or((0, 0, false))
}

// Diagnostics use only the current application Meeting authority; never
// return private turns, identities, or wall-clock speech timestamps.
pub(super) fn current_outbound_latency_distribution() -> Option<(usize, u64, u64)> {
    let session = latest_runtime_session_state().snapshot?;
    if session.owner_id != APPLICATION_MEETING_OWNER_ID {
        return None;
    }
    let guard = committed_turn_store().lock().ok()?;
    let store = guard.as_ref()?;
    if store.session_id != session.session_id {
        return None;
    }
    store.outbound_latency_distribution()
}

pub(super) fn current_committed_turn_snapshot() -> MeetingCommittedTurnsSnapshot {
    let session = latest_runtime_session_state().snapshot;
    let Some(session) = session else {
        return empty_committed_turn_snapshot(
            false,
            None,
            "",
            "No application Meeting session currently owns committed transcript turns.",
        );
    };
    if session.owner_id != APPLICATION_MEETING_OWNER_ID {
        return empty_committed_turn_snapshot(
            true,
            Some(session.session_id),
            "meeting_committed_turns:owner_conflict",
            "Committed Meeting turns are unavailable because another runtime owner holds the active session.",
        );
    }

    let session_id = session.session_id;
    let Ok(guard) = committed_turn_store().lock() else {
        return empty_committed_turn_snapshot(
            true,
            Some(session_id),
            "meeting_committed_turns:state_lock_failed",
            "Committed Meeting turn state is temporarily unavailable.",
        );
    };
    let Some(store) = guard.as_ref() else {
        return empty_committed_turn_snapshot(
            true,
            Some(session_id),
            "",
            "The active Meeting session has not committed any translated turns yet.",
        );
    };
    if store.session_id != session_id {
        return empty_committed_turn_snapshot(
            true,
            Some(session_id),
            "meeting_committed_turns:session_mismatch",
            "Committed Meeting turn state does not belong to the current application Meeting session.",
        );
    }

    store.snapshot()
}

#[cfg(test)]
mod tests {
    use super::{
        MeetingCommittedTurnStore, MeetingCommittedTurnsSnapshot, MAX_LIVE_COMMITTED_TURNS,
        MAX_SESSION_COMMITTED_TURNS, MAX_SESSION_TRANSCRIPT_BYTES,
    };
    use crate::commands::meeting_session::MeetingOutboundTiming;
    use crate::engine::runtime_state::runtime_test_guard;

    fn timing(finalized_unix_ms: u128) -> MeetingOutboundTiming {
        MeetingOutboundTiming {
            finalized_unix_ms,
            first_playback_unix_ms: None,
            speech_boundary_ms: 10,
            finalization_ms: 2,
            queue_ms: 3,
            audio_prepare_ms: 4,
            asr_ms: Some(5),
            translation_ms: Some(6),
            translation_tokenization_ms: Some(1.0),
            translation_inference_ms: Some(4.0),
            translation_decode_ms: Some(1.0),
            translation_tokens_per_second: Some(50.0),
            tts_ms: Some(7),
            playback_queue_ms: None,
            delivery_ms: None,
            outbound_latency_ms: None,
            captured_speech_span_ms: 1_000,
            estimated_onset_to_first_audio_ms: None,
            estimated_end_to_first_audio_ms: None,
            asr_scheduler_wait_ms: None,
            translation_scheduler_wait_ms: None,
            tts_scheduler_wait_ms: None,
            output_stream_build_ms: None,
            output_stream_start_to_first_audio_ms: None,
        }
    }

    #[test]
    fn source_first_turn_keeps_original_when_translation_fails() {
        let mut store = MeetingCommittedTurnStore::new("s1");
        assert!(store.commit_at(
            "s1", 1, Some(7), 51, "you", "id", "en",
            "Saya tidak setuju", "", Some("translating"), Some(timing(1)), 1_000,
        ).is_some());
        assert_eq!(store.turns.front().unwrap().translation_state, "pending");
        assert!(store.finish_translation_at("s1", 1, Some(7), 51, "you", None, 1_050).is_some());
        let turn = store.turns.front().unwrap();
        assert_eq!(turn.source_text, "Saya tidak setuju");
        assert_eq!(turn.translated_text, "");
        assert_eq!(turn.translation_state, "failed");
        assert_eq!(turn.delivery_state.as_deref(), Some("output_failed"));
        assert!(store.finish_translation_at(
            "s1", 1, Some(7), 51, "you", Some("I agree"), 1_100,
        ).is_none());
        assert_eq!(store.turns.front().unwrap().source_text, "Saya tidak setuju");
    }

    #[test]
    fn delayed_incoming_translation_is_bound_to_source_and_session() {
        let mut store = MeetingCommittedTurnStore::new("s1");
        assert!(store.commit_at(
            "s1", 5, None, 31, "incoming", "en", "id", "Please confirm", "",
            None, None, 5_000,
        ).is_some());
        assert!(store.finish_translation_at(
            "s2", 5, None, 31, "incoming", Some("Mohon konfirmasi"), 5_001,
        ).is_none());
        assert!(store.finish_translation_at(
            "s1", 5, None, 32, "incoming", Some("Mohon konfirmasi"), 5_002,
        ).is_none());
        assert!(store.finish_translation_at(
            "s1", 5, None, 31, "incoming", Some("Mohon konfirmasi"), 5_003,
        ).is_some());
        let turn = store.turns.front().unwrap();
        assert_eq!(turn.translation_state, "translated");
        assert_eq!(turn.translated_text, "Mohon konfirmasi");
        assert_eq!(turn.source_text, "Please confirm");
        assert!(store.finish_translation_at("s1", 5, None, 31, "incoming", None, 5_004).is_none());
    }

    #[test]
    fn interrupted_source_first_turn_cannot_receive_late_voice() {
        let mut store = MeetingCommittedTurnStore::new("s1");
        assert!(store.commit_at(
            "s1", 1, Some(7), 51, "you", "id", "en",
            "Tunggu sebentar", "", Some("translating"), Some(timing(1)), 1_000,
        ).is_some());
        store.interrupt_generation_at(7, 1_010);
        assert_eq!(store.turns.front().unwrap().translation_state, "failed");
        assert_eq!(store.turns.front().unwrap().delivery_state.as_deref(), Some("interrupted"));
        assert!(store.finish_translation_at(
            "s1", 1, Some(7), 51, "you", Some("Wait a moment"), 1_020,
        ).is_none());
    }

    #[test]
    fn latency_distribution_excludes_incoming_failed_and_unmeasured_turns() {
        let mut store = MeetingCommittedTurnStore::new("session-a");
        assert_eq!(store.outbound_latency_distribution(), None);
        for (sequence, state, latency) in [
            (1_u64, "output_complete", Some(10_u64)),
            (2, "output_failed", Some(900)),
            (3, "output_complete", Some(20)),
            (4, "output_complete", None),
            (5, "output_complete", Some(30)),
            (6, "interrupted", Some(1000)),
        ] {
            let mut observed = timing(u128::from(sequence));
            observed.outbound_latency_ms = latency;
            assert!(store.commit_at(
                "session-a", sequence, Some(9), sequence, "you", "id", "en",
                "source", "translation", Some(state), Some(observed),
                u128::from(sequence),
            ).is_some());
        }
        assert!(store.commit_at(
            "session-a", 7, None, 7, "incoming", "en", "id",
            "message", "pesan", None, None, 7,
        ).is_some());
        assert_eq!(store.outbound_latency_distribution(), Some((3, 20, 30)));
    }

    #[test]
    fn committed_turns_remain_in_shared_event_order() {
        let mut store = MeetingCommittedTurnStore::new("session-a");
        assert!(store.commit_at(
            "session-a",
            3,
            Some(9),
            30,
            "you",
            "id",
            "en",
            "tiga",
            "three",
            Some("preparing_voice"),
            Some(timing(3)),
            1_003,
        ).is_some());
        assert!(store.commit_at(
            "session-a",
            1,
            None,
            10,
            "incoming",
            "en",
            "id",
            "one",
            "satu",
            None,
            None,
            1_001,
        ).is_some());
        assert!(store.commit_at(
            "session-a",
            2,
            Some(9),
            20,
            "you",
            "id",
            "en",
            "dua",
            "two",
            Some("preparing_voice"),
            Some(timing(2)),
            1_002,
        ).is_some());

        let sequences = store.turns.iter().map(|turn| turn.sequence).collect::<Vec<_>>();
        assert_eq!(sequences, vec![1, 2, 3]);
    }

    #[test]
    fn repeated_no_session_cleanup_preserves_last_ended_export() {
        // Reuse the repository's single global-runtime fixture guard. Restore
        // both stores even if an assertion panics, so parallel tests cannot
        // inherit an unrelated transcript or a poisoned fixture state.
        let _serial = runtime_test_guard();
        struct RestoreStores {
            active: Option<MeetingCommittedTurnStore>,
            ended: Option<MeetingCommittedTurnsSnapshot>,
        }
        impl Drop for RestoreStores {
            fn drop(&mut self) {
                *super::committed_turn_store()
                    .lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = self.active.take();
                *super::ended_transcript_store()
                    .lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = self.ended.take();
            }
        }
        let mut active_store = super::committed_turn_store().lock().unwrap();
        let mut ended_store = super::ended_transcript_store().lock().unwrap();
        let _restore = RestoreStores {
            active: active_store.take(),
            ended: ended_store.take(),
        };
        drop(ended_store);
        drop(active_store);

        let session_id = "idempotent-stop-transcript-fixture";
        let mut active = MeetingCommittedTurnStore::new(session_id);
        assert!(active.commit_at(
            session_id, 1, None, 11, "incoming", "en", "id",
            "Hello", "Halo", None, None, 1,
        ).is_some());
        *super::committed_turn_store().lock().unwrap() = Some(active);

        assert!(super::retain_committed_turns_for_export(session_id));
        assert!(super::clear_committed_turns_for_session(session_id));
        assert!(super::clear_inactive_committed_turns());
        assert!(super::clear_inactive_committed_turns());

        let archived = super::ended_transcript_store().lock().unwrap().clone()
            .expect("second Stop must preserve the last-ended transcript");
        assert_eq!(archived.session_id.as_deref(), Some(session_id));
        assert_eq!(archived.turns.len(), 1);
        assert_eq!(archived.turns[0].source_text, "Hello");
        assert_eq!(archived.turns[0].translated_text, "Halo");

        // RAII restoration runs before the global-runtime fixture guard
        // releases, including on a failing assertion.
    }

    #[test]
    fn committed_turn_retention_drops_only_oldest_events() {
        let mut store = MeetingCommittedTurnStore::new("session-a");
        for sequence in 1..=(MAX_SESSION_COMMITTED_TURNS as u64 + 2) {
            assert!(store.commit_at(
                "session-a",
                sequence,
                None,
                sequence,
                "incoming",
                "en",
                "id",
                "english",
                "indonesia",
                None,
                None,
                u128::from(sequence),
            ).is_some());
        }

        assert_eq!(store.turns.len(), MAX_SESSION_COMMITTED_TURNS);
        assert_eq!(store.dropped_turn_count, 2);
        assert_eq!(store.turns.front().map(|turn| turn.sequence), Some(3));
        assert_eq!(
            store.turns.back().map(|turn| turn.sequence),
            Some(MAX_SESSION_COMMITTED_TURNS as u64 + 2)
        );
        assert_eq!(store.snapshot().turns.len(), MAX_LIVE_COMMITTED_TURNS);
        assert_eq!(store.snapshot_for_export().turns.len(), MAX_SESSION_COMMITTED_TURNS);
        assert!(store.snapshot_for_export().truncated);
    }

    #[test]
    fn recent_preview_is_short_but_manual_export_keeps_full_session() {
        let mut store = MeetingCommittedTurnStore::new("s1");
        for seq in 1..=(MAX_LIVE_COMMITTED_TURNS as u64 + 3) {
            assert!(store.commit_at(
                "s1", seq, None, seq, "incoming", "en", "id",
                "Can you confirm?", "Bisa konfirmasi?", None, None, seq as u128,
            ).is_some());
        }
        let preview = store.snapshot();
        let export = store.snapshot_for_export();
        assert_eq!(preview.turns.len(), MAX_LIVE_COMMITTED_TURNS);
        assert_eq!(preview.turns.first().map(|turn| turn.sequence), Some(4));
        assert!(!preview.truncated);
        assert_eq!(export.turns.len(), MAX_LIVE_COMMITTED_TURNS + 3);
        assert!(!export.truncated);
        assert_eq!(store.dropped_turn_count, 0);
    }

    #[test]
    fn memory_budget_is_enforced_even_with_fewer_than_turn_count_limit() {
        let mut store = MeetingCommittedTurnStore::new("s1");
        let oversized = "x".repeat(MAX_SESSION_TRANSCRIPT_BYTES + 1);
        assert!(store.commit_at(
            "s1", 1, None, 1, "incoming", "en", "id",
            &oversized, "", None, None, 1,
        ).is_some());
        assert!(store.turns.is_empty());
        assert_eq!(store.retained_text_bytes, 0);
        assert_eq!(store.dropped_turn_count, 1);
        assert!(store.snapshot_for_export().truncated);
    }

    #[test]
    fn archive_receives_new_source_even_when_memory_evicts_it_immediately() {
        let mut store = MeetingCommittedTurnStore::new("s1");
        let oversized = "x".repeat(MAX_SESSION_TRANSCRIPT_BYTES + 1);
        let accepted = store.commit_at(
            "s1", 1, None, 1, "incoming", "en", "id",
            &oversized, "", None, None, 1,
        ).expect("validated source must be handed to archive");
        assert_eq!(accepted.sequence, 1);
        assert_eq!(accepted.source_text, oversized);
        assert!(store.turns.is_empty(), "Live memory remains bounded");
        assert_eq!(store.dropped_turn_count, 1);
    }

    #[test]
    fn archive_receives_completed_translation_even_if_updated_turn_is_evicted() {
        let mut store = MeetingCommittedTurnStore::new("s1");
        assert!(store.commit_at(
            "s1", 1, None, 1, "incoming", "en", "id",
            "Source phrase", "", None, None, 1,
        ).is_some());
        let oversized_translation = "y".repeat(MAX_SESSION_TRANSCRIPT_BYTES + 1);
        let accepted = store.finish_translation_at(
            "s1", 1, None, 1, "incoming", Some(&oversized_translation), 2,
        ).expect("completed translation must be handed to archive");
        assert_eq!(accepted.sequence, 1);
        assert_eq!(accepted.translation_state, "translated");
        assert_eq!(accepted.translated_text, oversized_translation);
        assert!(store.turns.is_empty(), "Live memory remains bounded");
        assert_eq!(store.dropped_turn_count, 1);
    }

    #[test]
    fn stop_returns_only_newly_interrupted_turns_for_disk_update() {
        let mut store = MeetingCommittedTurnStore::new("session-a");
        assert!(store.commit_at(
            "session-a", 1, Some(7), 11, "you", "id", "en",
            "Halo", "", Some("translating"), Some(timing(100)), 100,
        ).is_some());
        assert!(store.commit_at(
            "session-a", 2, Some(7), 12, "you", "id", "en",
            "Terima kasih", "Thank you", Some("output_complete"), Some(timing(101)), 101,
        ).is_some());

        let changed = store.interrupt_generation_at(7, 200);
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].sequence, 1);
        assert_eq!(changed[0].delivery_state.as_deref(), Some("interrupted"));
        assert_eq!(changed[0].translation_state, "failed");
        assert_eq!(changed[0].updated_unix_ms, 200);
        assert!(store.interrupt_generation_at(7, 300).is_empty(),
            "Retry Stop must not enqueue a duplicate interruption");
        let completed = store.turns.iter().find(|turn| turn.sequence == 2).unwrap();
        assert_eq!(completed.delivery_state.as_deref(), Some("output_complete"));
    }

    #[test]
    fn final_playback_state_and_timing_remain_attached_to_one_turn() {
        let mut store = MeetingCommittedTurnStore::new("session-a");
        assert!(store.commit_at(
            "session-a", 5, Some(7), 51, "you", "id", "en",
            "Penting", "Important", Some("preparing_voice"), Some(timing(1_000)), 1_000,
        ).is_some());
        assert!(store.update_delivery_state_at("session-a", 7, 51, "queued", 1_010));
        assert!(store.begin_playback_at("session-a", 7, 5, 51, 1_020));
        assert!(store.update_delivery_state_at("session-a", 7, 51, "output_complete", 1_030));
        let mut measured = timing(1_000);
        measured.first_playback_unix_ms = Some(1_025);
        measured.outbound_latency_ms = Some(25);
        assert!(store.update_outbound_timing_at("session-a", 7, 51, &measured, 1_040));
        let turn = store.turns.front().unwrap();
        assert_eq!(turn.sequence, 5);
        assert_eq!(turn.delivery_state.as_deref(), Some("output_complete"));
        assert_eq!(turn.outbound_timing.as_ref().and_then(|t| t.outbound_latency_ms), Some(25));
        assert_eq!(turn.updated_unix_ms, 1_040);
    }

    #[test]
    fn terminal_delivery_state_cannot_be_rewritten() {
        let mut store = MeetingCommittedTurnStore::new("session-a");
        assert!(store.commit_at(
            "session-a",
            1,
            Some(7),
            11,
            "you",
            "id",
            "en",
            "halo",
            "hello",
            Some("preparing_voice"),
            Some(timing(1)),
            1_000,
        ).is_some());
        assert!(store.update_delivery_state_at("session-a", 7, 11, "speaking", 1_010));
        assert!(store.update_delivery_state_at(
            "session-a",
            7,
            11,
            "output_complete",
            1_020,
        ));
        assert!(!store.update_delivery_state_at(
            "session-a",
            7,
            11,
            "output_failed",
            1_030,
        ));
        assert!(store.update_delivery_state_at(
            "session-a",
            7,
            11,
            "output_complete",
            1_040,
        ));

        assert_eq!(
            store.turns.front().and_then(|turn| turn.delivery_state.as_deref()),
            Some("output_complete")
        );
        assert_eq!(store.turns.front().map(|turn| turn.updated_unix_ms), Some(1_020));
    }

    #[test]
    fn duplicate_source_and_evicted_sequence_never_restart_inference() {
        let mut store = MeetingCommittedTurnStore::new("s1");
        assert!(store.commit_at(
            "s1", 1, Some(7), 10, "you", "id", "en",
            "Halo", "", Some("translating"), Some(timing(1)), 1,
        ).is_some());
        assert!(store.commit_at(
            "s1", 1, Some(7), 10, "you", "id", "en",
            "Halo", "", Some("translating"), Some(timing(1)), 2,
        ).is_none());
        assert!(store.commit_at(
            "s1", 1, Some(7), 11, "you", "id", "en",
            "Sumber lain", "", Some("translating"), Some(timing(1)), 3,
        ).is_none());
        assert!(store.commit_at(
            "s1", 2, Some(7), 10, "you", "id", "en",
            "Halo", "", Some("translating"), Some(timing(2)), 4,
        ).is_none());
        for sequence in 2..=(MAX_SESSION_COMMITTED_TURNS as u64 + 2) {
            assert!(store.commit_at(
                "s1", sequence, None, sequence, "incoming", "en", "id",
                "input", "", None, None, u128::from(sequence),
            ).is_some());
        }
        assert!(store.retired_sequence >= 1);
        assert!(store.commit_at(
            "s1", 1, Some(7), 10, "you", "id", "en",
            "Halo", "", Some("translating"), Some(timing(1)), 10_000,
        ).is_none());
    }

    #[test]
    fn playback_requires_locked_queued_source_with_exact_sequence() {
        let mut store = MeetingCommittedTurnStore::new("s1");
        assert!(store.commit_at(
            "s1", 3, Some(7), 10, "you", "id", "en",
            "Lanjutkan", "", Some("translating"), Some(timing(3)), 1,
        ).is_some());
        assert!(!store.update_delivery_state_at("s1", 7, 10, "queued", 2));
        assert!(!store.begin_playback_at("s1", 7, 3, 10, 3));
        assert!(store.finish_translation_at("s1", 3, Some(7), 10, "you", Some("Continue"), 4).is_some());
        assert!(!store.begin_playback_at("s1", 7, 4, 10, 5));
        assert!(!store.begin_playback_at("s1", 8, 3, 10, 5));
        assert!(store.update_delivery_state_at("s1", 7, 10, "queued", 6));
        assert!(store.begin_playback_at("s1", 7, 3, 10, 7));
        assert!(!store.begin_playback_at("s1", 7, 3, 10, 8));
        assert!(!store.update_delivery_state_at("s1", 7, 10, "queued", 9));
    }

}
