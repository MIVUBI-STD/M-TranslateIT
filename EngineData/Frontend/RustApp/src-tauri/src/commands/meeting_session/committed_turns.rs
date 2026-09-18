use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use crate::engine::runtime_state::latest_runtime_session_state;

use super::super::helper_bridge_runtime::unix_ms;
use super::{
    MeetingCommittedTurn, MeetingCommittedTurnsSnapshot, MeetingOutboundTiming,
    APPLICATION_MEETING_OWNER_ID,
};

const MAX_LIVE_COMMITTED_TURNS: usize = 240;

struct MeetingCommittedTurnStore {
    session_id: String,
    dropped_turn_count: u64,
    turns: VecDeque<MeetingCommittedTurn>,
}

impl MeetingCommittedTurnStore {
    fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            dropped_turn_count: 0,
            turns: VecDeque::new(),
        }
    }

    fn recent_outbound_context_pairs(&self, max_pairs: usize) -> Vec<[String; 2]> {
        let mut newest_first: Vec<[String; 2]> = self
            .turns
            .iter()
            .rev()
            .filter(|turn| turn.lane == "you")
            .take(max_pairs)
            .map(|turn| [turn.source_text.clone(), turn.translated_text.clone()])
            .collect();
        newest_first.reverse();
        newest_first
    }

    #[allow(clippy::too_many_arguments)]
    fn commit_at(
        &mut self,
        session_id: &str,
        sequence: u64,
        generation: Option<u64>,
        utterance_id: u64,
        lane: &str,
        source_text: &str,
        translated_text: &str,
        delivery_state: Option<&str>,
        outbound_timing: Option<MeetingOutboundTiming>,
        now_unix_ms: u128,
    ) -> bool {
        if self.session_id != session_id || sequence == 0 || !matches!(lane, "you" | "incoming") {
            return false;
        }
        if lane == "you"
            && (generation.is_none() || delivery_state.is_none() || outbound_timing.is_none())
        {
            return false;
        }
        if lane == "incoming"
            && (generation.is_some() || delivery_state.is_some() || outbound_timing.is_some())
        {
            return false;
        }
        if self
            .turns
            .iter()
            .any(|turn| turn.session_id == session_id && turn.sequence == sequence)
        {
            return true;
        }

        let turn = MeetingCommittedTurn {
            session_id: session_id.to_string(),
            sequence,
            generation,
            utterance_id,
            lane: lane.to_string(),
            source_text: source_text.to_string(),
            translated_text: translated_text.to_string(),
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
        self.turns.insert(insert_at, turn);

        while self.turns.len() > MAX_LIVE_COMMITTED_TURNS {
            let _ = self.turns.pop_front();
            self.dropped_turn_count = self.dropped_turn_count.saturating_add(1);
        }
        true
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
                "preparing_voice"
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
        turn.delivery_state = Some(delivery_state.to_string());
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

    fn interrupt_generation_at(&mut self, generation: u64, now_unix_ms: u128) {
        for turn in &mut self.turns {
            if turn.generation == Some(generation)
                && !terminal_delivery_state(turn.delivery_state.as_deref())
            {
                turn.delivery_state = Some("interrupted".to_string());
                turn.updated_unix_ms = now_unix_ms;
            }
        }
    }

    fn snapshot(&self) -> MeetingCommittedTurnsSnapshot {
        let mut turns = self.turns.iter().cloned().collect::<Vec<_>>();
        turns.sort_by_key(|turn| turn.sequence);
        MeetingCommittedTurnsSnapshot {
            ok: true,
            has_session: true,
            session_id: Some(self.session_id.clone()),
            turns,
            dropped_turn_count: self.dropped_turn_count,
            truncated: self.dropped_turn_count > 0,
            blocker: String::new(),
            note: if self.dropped_turn_count > 0 {
                format!(
                    "Live transcript snapshot is bounded; {} earlier committed turns are no longer retained in the transient view.",
                    self.dropped_turn_count
                )
            } else {
                "Live transcript snapshot contains the currently retained committed turns for this Meeting session in finalized speech/event order."
                    .to_string()
            },
            runtime_claim: "meeting_committed_turn_snapshot_source_contract_not_rendered_runtime_proof"
                .to_string(),
        }
    }
}

static MEETING_COMMITTED_TURNS: OnceLock<Mutex<Option<MeetingCommittedTurnStore>>> =
    OnceLock::new();

fn committed_turn_store() -> &'static Mutex<Option<MeetingCommittedTurnStore>> {
    MEETING_COMMITTED_TURNS.get_or_init(|| Mutex::new(None))
}

fn terminal_delivery_state(state: Option<&str>) -> bool {
    matches!(
        state,
        Some("output_complete" | "output_failed" | "interrupted")
    )
}

pub(super) fn reset_committed_turns(session_id: &str) {
    if let Ok(mut guard) = committed_turn_store().lock() {
        *guard = Some(MeetingCommittedTurnStore::new(session_id));
    }
}

pub(super) fn clear_committed_turns_for_session(session_id: &str) -> bool {
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

pub(super) fn clear_all_committed_turns() -> bool {
    let Ok(mut guard) = committed_turn_store().lock() else {
        return false;
    };
    *guard = None;
    true
}

pub(super) fn recent_outbound_context_pairs(
    session_id: &str,
    max_pairs: usize,
) -> Vec<[String; 2]> {
    let Ok(guard) = committed_turn_store().lock() else {
        return Vec::new();
    };
    let Some(store) = guard.as_ref() else {
        return Vec::new();
    };
    if store.session_id != session_id {
        return Vec::new();
    }
    store.recent_outbound_context_pairs(max_pairs)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn commit_meeting_turn(
    session_id: &str,
    sequence: u64,
    generation: Option<u64>,
    utterance_id: u64,
    lane: &str,
    source_text: &str,
    translated_text: &str,
    delivery_state: Option<&str>,
    outbound_timing: Option<MeetingOutboundTiming>,
) -> bool {
    let Ok(mut guard) = committed_turn_store().lock() else {
        return false;
    };
    let Some(store) = guard.as_mut() else {
        return false;
    };
    store.commit_at(
        session_id,
        sequence,
        generation,
        utterance_id,
        lane,
        source_text,
        translated_text,
        delivery_state,
        outbound_timing,
        unix_ms(),
    )
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
    store.update_delivery_state_at(
        session_id,
        generation,
        utterance_id,
        delivery_state,
        unix_ms(),
    )
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
    store.update_outbound_timing_at(session_id, generation, utterance_id, timing, unix_ms())
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
    store.interrupt_generation_at(generation, unix_ms());
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
        truncated: false,
        blocker: blocker.to_string(),
        note: note.to_string(),
        runtime_claim: "meeting_committed_turn_snapshot_source_contract_not_rendered_runtime_proof"
            .to_string(),
    }
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
    use super::{MeetingCommittedTurnStore, MAX_LIVE_COMMITTED_TURNS};
    use crate::commands::meeting_session::MeetingOutboundTiming;

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
            delivery_ms: None,
            outbound_latency_ms: None,
        }
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
            "tiga",
            "three",
            Some("preparing_voice"),
            Some(timing(3)),
            1_003,
        ));
        assert!(store.commit_at(
            "session-a",
            1,
            None,
            10,
            "incoming",
            "one",
            "satu",
            None,
            None,
            1_001,
        ));
        assert!(store.commit_at(
            "session-a",
            2,
            Some(9),
            20,
            "you",
            "dua",
            "two",
            Some("preparing_voice"),
            Some(timing(2)),
            1_002,
        ));

        let sequences = store.turns.iter().map(|turn| turn.sequence).collect::<Vec<_>>();
        assert_eq!(sequences, vec![1, 2, 3]);
    }

    #[test]
    fn committed_turn_retention_drops_only_oldest_events() {
        let mut store = MeetingCommittedTurnStore::new("session-a");
        for sequence in 1..=(MAX_LIVE_COMMITTED_TURNS as u64 + 2) {
            assert!(store.commit_at(
                "session-a",
                sequence,
                None,
                sequence,
                "incoming",
                "english",
                "indonesia",
                None,
                None,
                u128::from(sequence),
            ));
        }

        assert_eq!(store.turns.len(), MAX_LIVE_COMMITTED_TURNS);
        assert_eq!(store.dropped_turn_count, 2);
        assert_eq!(store.turns.front().map(|turn| turn.sequence), Some(3));
        assert_eq!(
            store.turns.back().map(|turn| turn.sequence),
            Some(MAX_LIVE_COMMITTED_TURNS as u64 + 2)
        );
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
            "halo",
            "hello",
            Some("preparing_voice"),
            Some(timing(1)),
            1_000,
        ));
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
    fn outbound_context_uses_latest_own_turns_in_chronological_order() {
        let mut store = MeetingCommittedTurnStore::new("session-a");
        assert!(store.commit_at(
            "session-a",
            1,
            Some(5),
            10,
            "you",
            "satu",
            "one",
            Some("output_complete"),
            Some(timing(1)),
            1,
        ));
        assert!(store.commit_at(
            "session-a",
            2,
            None,
            20,
            "incoming",
            "incoming",
            "masuk",
            None,
            None,
            2,
        ));
        assert!(store.commit_at(
            "session-a",
            3,
            Some(5),
            30,
            "you",
            "tiga",
            "three",
            Some("output_complete"),
            Some(timing(3)),
            3,
        ));
        assert!(store.commit_at(
            "session-a",
            4,
            Some(5),
            40,
            "you",
            "empat",
            "four",
            Some("output_complete"),
            Some(timing(4)),
            4,
        ));

        assert_eq!(
            store.recent_outbound_context_pairs(2),
            vec![
                ["tiga".to_string(), "three".to_string()],
                ["empat".to_string(), "four".to_string()],
            ]
        );
    }
}
