use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::engine::audio::live_segment_writer::remove_finalized_meeting_utterance_wav;

use super::{worker_blocker, worker_text};
use super::committed_turns::finish_meeting_turn_translation;
use super::super::helper_bridge::HelperBridgeWorkerResponse;

pub(super) const MAX_DEFERRED_INCOMING: usize = 4;
const MAX_DEFERRED_INCOMING_AGE_MS: u64 = 20_000;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum IncomingAsrDisposition {
    Deferred,
    EmptyTranscript,
    Failed(String),
    Transcript(String),
}

#[derive(Clone)]
pub(super) enum DeferredIncomingStage {
    NeedsAsr {
        audio_path: String,
        source_language: String,
        target_language: String,
    },
    NeedsTranslation {
        transcript: String,
        source_language: String,
        target_language: String,
    },
}

pub(super) struct DeferredIncomingJob {
    pub(super) session_id: String,
    pub(super) event_sequence: u64,
    pub(super) utterance_id: u64,
    pub(super) stage: DeferredIncomingStage,
    // First deferral time. Requeues must retain this value so the 20-second
    // bounded age budget cannot be refreshed by repeated outbound preemption.
    pub(super) enqueued_unix_ms: u64,
}

static DEFERRED_INCOMING_QUEUE: OnceLock<Mutex<VecDeque<DeferredIncomingJob>>> = OnceLock::new();
static DEFERRED_DROPPED_OVERFLOW: AtomicU64 = AtomicU64::new(0);
static DEFERRED_DROPPED_STALE: AtomicU64 = AtomicU64::new(0);

fn deferred_incoming_queue() -> &'static Mutex<VecDeque<DeferredIncomingJob>> {
    DEFERRED_INCOMING_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub(super) fn incoming_deferred_for_required_outbound(
    response: &HelperBridgeWorkerResponse,
) -> bool {
    worker_text(response, "blocker").as_deref()
        == Some("helper_scheduler:incoming_deferred_for_outbound")
}

pub(super) fn classify_incoming_asr_response(
    asr: &HelperBridgeWorkerResponse,
) -> IncomingAsrDisposition {
    if incoming_deferred_for_required_outbound(asr) {
        return IncomingAsrDisposition::Deferred;
    }

    let transcript = worker_text(asr, "transcript_text");
    if !asr.ok || transcript.is_none() {
        let blocker = worker_blocker(asr, "asr:empty_transcript");
        if blocker.contains("empty_transcript") {
            IncomingAsrDisposition::EmptyTranscript
        } else {
            IncomingAsrDisposition::Failed(blocker)
        }
    } else {
        IncomingAsrDisposition::Transcript(transcript.unwrap_or_default())
    }
}

pub(super) fn deferred_enqueue_unix_ms(
    first_deferred_unix_ms: Option<u64>,
    now_unix_ms: u64,
) -> u64 {
    first_deferred_unix_ms.unwrap_or(now_unix_ms)
}

fn cleanup_deferred_incoming_job(job: DeferredIncomingJob) {
    match job.stage {
        DeferredIncomingStage::NeedsAsr { audio_path, .. } => {
            remove_finalized_meeting_utterance_wav(&audio_path);
        }
        DeferredIncomingStage::NeedsTranslation { .. } => {
            // A stable ASR source has already been committed. Expiration or
            // queue eviction must not leave its translation forever "pending".
            let _ = finish_meeting_turn_translation(
                &job.session_id, job.event_sequence, None, job.utterance_id,
                "incoming", None,
            );
        }
    }
}

// Serialize admission with queue mutation and cleanup: a worker that resumes
// after suppression/Stop must not refill a queue that was already cleared.
fn enqueue_with_admission(
    job: DeferredIncomingJob,
    front: bool,
    admitted: impl FnOnce(&str) -> bool,
) -> usize {
    let Ok(mut guard) = deferred_incoming_queue().lock() else {
        cleanup_deferred_incoming_job(job);
        return 0;
    };
    // Admission and state mutation remain serialized with Stop/queue clear.
    // All file cleanup and transcript updates happen AFTER releasing the lock.
    if !admitted(&job.session_id) {
        drop(guard);
        cleanup_deferred_incoming_job(job);
        return 0;
    }
    if front {
        guard.push_front(job);
    } else {
        guard.push_back(job);
    }
    let mut evicted_jobs = Vec::new();
    while guard.len() > MAX_DEFERRED_INCOMING {
        let evicted = if front { guard.pop_back() } else { guard.pop_front() };
        if let Some(evicted) = evicted {
            evicted_jobs.push(evicted);
        }
        DEFERRED_DROPPED_OVERFLOW.fetch_add(1, Ordering::Relaxed);
    }
    let len = guard.len();
    drop(guard);
    for evicted in evicted_jobs {
        cleanup_deferred_incoming_job(evicted);
    }
    len
}

pub(super) fn enqueue_deferred_incoming(job: DeferredIncomingJob) -> usize {
    enqueue_with_admission(job, false, super::incoming_session_is_eligible)
}

pub(super) fn requeue_deferred_incoming_front(job: DeferredIncomingJob) -> usize {
    enqueue_with_admission(job, true, super::incoming_session_is_eligible)
}

pub(super) fn take_due_deferred_incoming(
    session_id: &str,
    now_unix_ms: u64,
) -> Option<DeferredIncomingJob> {
    let mut guard = deferred_incoming_queue().lock().ok()?;
    let mut removed = Vec::new();
    let due = loop {
        let Some(front) = guard.front() else {
            break None;
        };
        if front.session_id != session_id
            || now_unix_ms.saturating_sub(front.enqueued_unix_ms) > MAX_DEFERRED_INCOMING_AGE_MS
        {
            if let Some(stale) = guard.pop_front() {
                removed.push(stale);
                DEFERRED_DROPPED_STALE.fetch_add(1, Ordering::Relaxed);
            }
            continue;
        }
        break guard.pop_front();
    };
    drop(guard);
    for stale in removed {
        cleanup_deferred_incoming_job(stale);
    }
    due
}

pub(super) fn reset_deferred_incoming_drop_counters() {
    DEFERRED_DROPPED_OVERFLOW.store(0, Ordering::Relaxed);
    DEFERRED_DROPPED_STALE.store(0, Ordering::Relaxed);
}

pub(super) fn deferred_incoming_health_counts() -> (usize, u64, u64) {
    let depth = deferred_incoming_queue()
        .lock()
        .map(|guard| guard.len())
        .unwrap_or(MAX_DEFERRED_INCOMING);
    (
        depth,
        DEFERRED_DROPPED_OVERFLOW.load(Ordering::Relaxed),
        DEFERRED_DROPPED_STALE.load(Ordering::Relaxed),
    )
}

// Remove only the stopped session's jobs. A delayed Stop must not evict a
// newer Meeting's deferred work; cleanup of removed jobs runs after unlock.
fn drain_session_jobs(
    queue: &mut VecDeque<DeferredIncomingJob>,
    session_id: &str,
) -> Vec<DeferredIncomingJob> {
    let mut retained = VecDeque::with_capacity(queue.len());
    let mut removed = Vec::new();
    while let Some(job) = queue.pop_front() {
        if job.session_id == session_id {
            removed.push(job);
        } else {
            retained.push_back(job);
        }
    }
    *queue = retained;
    removed
}

pub(super) fn clear_deferred_incoming_queue_for_session(session_id: &str) -> Result<(), String> {
    if session_id.is_empty() {
        return Err("meeting_incoming:deferred_queue_session_missing".to_string());
    }
    let mut guard = deferred_incoming_queue()
        .lock()
        .map_err(|_| "meeting_incoming:deferred_queue_lock_failed".to_string())?;
    let removed = drain_session_jobs(&mut guard, session_id);
    drop(guard);
    for job in removed {
        cleanup_deferred_incoming_job(job);
    }
    Ok(())
}

pub(super) fn clear_deferred_incoming_queue() -> Result<(), String> {
    let mut guard = deferred_incoming_queue()
        .lock()
        .map_err(|_| "meeting_incoming:deferred_queue_lock_failed".to_string())?;
    let jobs: Vec<_> = guard.drain(..).collect();
    drop(guard);
    for job in jobs {
        cleanup_deferred_incoming_job(job);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(session: &str, seq: u64, transcript: &str, enqueued_unix_ms: u64) -> DeferredIncomingJob {
        DeferredIncomingJob {
            session_id: session.to_string(),
            event_sequence: seq,
            utterance_id: seq,
            stage: DeferredIncomingStage::NeedsTranslation {
                transcript: transcript.to_string(),
                source_language: "en".to_string(),
                target_language: "id".to_string(),
            },
            enqueued_unix_ms,
        }
    }

    // Each queue scenario mutates the same process-global deferred queue and
    // drop counters. Run them sequentially rather than as parallel Rust tests.
    #[test]
    fn deferred_queue_state_contracts_are_serialized() {
        deferred_admission_serializes_with_cleanup();
        deferred_retry_preserves_first_deferral_age_budget();
        deferred_queue_caps_evicts_expired_and_respects_session();
    }

    #[test]
    fn deferred_stop_drains_only_matching_session_and_preserves_foreign_fifo() {
        // Local fixture: no process-wide deferred queue or shared counters.
        let mut pending = VecDeque::from([
            job("next", 1, "keep first", 10),
            job("previous", 2, "remove", 11),
            job("next", 3, "keep second", 12),
            job("previous", 4, "remove", 13),
        ]);
        let removed = drain_session_jobs(&mut pending, "previous");
        assert_eq!(
            removed.iter().map(|item| item.event_sequence).collect::<Vec<_>>(),
            vec![2, 4],
        );
        assert_eq!(
            pending.iter().map(|item| item.event_sequence).collect::<Vec<_>>(),
            vec![1, 3],
            "a late Stop must retain the next session's order and work",
        );
        assert!(drain_session_jobs(&mut pending, "previous").is_empty());
        assert_eq!(pending.len(), 2, "retry Stop must be idempotent");
        assert!(drain_session_jobs(&mut pending, "absent").is_empty());
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn incoming_asr_deferral_is_classified_before_missing_transcript() {
        let response = HelperBridgeWorkerResponse {
            ok: false,
            state: "deferred".to_string(),
            task: "transcribe".to_string(),
            request_id: "test-incoming-asr-deferred".to_string(),
            scheduler_priority: "meeting_incoming".to_string(),
            message: "deferred".to_string(),
            generation_token: 1,
            runtime_claim: "test".to_string(),
            worker_response_json: serde_json::json!({
                "ok": false,
                "stage": "transcribe",
                "blocker": "helper_scheduler:incoming_deferred_for_outbound",
            })
            .to_string(),
        };

        assert_eq!(
            classify_incoming_asr_response(&response),
            IncomingAsrDisposition::Deferred
        );
    }

    fn deferred_admission_serializes_with_cleanup() {
        clear_deferred_incoming_queue().expect("clear deferred queue");
        let rejected = enqueue_with_admission(
            job("old", 1, "stale", 1_000),
            false,
            |session_id| {
                assert_eq!(session_id, "old");
                assert!(deferred_incoming_queue().try_lock().is_err());
                false
            },
        );
        assert_eq!(rejected, 0);
        assert_eq!(deferred_incoming_health_counts().0, 0);

        assert_eq!(
            enqueue_with_admission(job("current", 2, "live", 2_000), false, |_| true),
            1
        );
        clear_deferred_incoming_queue().expect("clear accepted work");
        assert_eq!(
            enqueue_with_admission(job("old", 3, "late", 3_000), true, |_| false),
            0
        );
        assert_eq!(deferred_incoming_health_counts().0, 0);
    }

    fn deferred_retry_preserves_first_deferral_age_budget() {
        clear_deferred_incoming_queue().expect("clear deferred queue");
        let first_deferred_unix_ms = 10_000;
        let retry_unix_ms = 29_000;
        let preserved = deferred_enqueue_unix_ms(Some(first_deferred_unix_ms), retry_unix_ms);
        assert_eq!(preserved, first_deferred_unix_ms);
        assert_eq!(deferred_enqueue_unix_ms(None, retry_unix_ms), retry_unix_ms);

        enqueue_with_admission(job("sess", 1, "t", preserved), true, |_| true);
        let stale_before = DEFERRED_DROPPED_STALE.load(Ordering::Relaxed);
        assert!(
            take_due_deferred_incoming(
                "sess",
                first_deferred_unix_ms + MAX_DEFERRED_INCOMING_AGE_MS + 1,
            )
            .is_none(),
            "requeued work must expire from its first deferral time"
        );
        assert_eq!(
            DEFERRED_DROPPED_STALE.load(Ordering::Relaxed) - stale_before,
            1
        );
        clear_deferred_incoming_queue().expect("clear deferred queue");
    }

    fn deferred_queue_caps_evicts_expired_and_respects_session() {
        clear_deferred_incoming_queue().expect("clear deferred queue");
        assert!(deferred_incoming_queue().lock().unwrap().is_empty());

        enqueue_with_admission(job("sess", 1, "t", 1_000), false, |_| true);
        let stale_before = DEFERRED_DROPPED_STALE.load(Ordering::Relaxed);
        assert!(take_due_deferred_incoming("other-sess", 2_000).is_none());
        assert_eq!(
            DEFERRED_DROPPED_STALE.load(Ordering::Relaxed) - stale_before,
            1,
            "foreign-session job must be treated as stale"
        );

        for seq in 2..=7 {
            enqueue_with_admission(job("sess", seq, "t", 10_000), false, |_| true);
        }
        assert_eq!(
            deferred_incoming_queue().lock().unwrap().len(),
            MAX_DEFERRED_INCOMING
        );
        let dropped_overflow = DEFERRED_DROPPED_OVERFLOW.swap(0, Ordering::Relaxed);
        assert_eq!(dropped_overflow, 2, "oldest two jobs must be evicted");

        let stale_before = DEFERRED_DROPPED_STALE.load(Ordering::Relaxed);
        let mut delivered = 0;
        while take_due_deferred_incoming("sess", 40_000).is_some() {
            delivered += 1;
        }
        assert_eq!(delivered, 0, "expired jobs are dropped, not delivered");
        assert_eq!(
            DEFERRED_DROPPED_STALE.load(Ordering::Relaxed) - stale_before,
            MAX_DEFERRED_INCOMING as u64
        );

        enqueue_with_admission(job("sess", 7, "a", 50_000), false, |_| true);
        enqueue_with_admission(job("sess", 8, "b", 51_000), false, |_| true);
        let first = take_due_deferred_incoming("sess", 52_000).expect("fresh job due");
        let second = take_due_deferred_incoming("sess", 52_000).expect("second fresh job due");
        assert_eq!(first.event_sequence, 7);
        assert_eq!(second.event_sequence, 8);

        clear_deferred_incoming_queue().expect("clear deferred queue");
        assert!(deferred_incoming_queue().lock().unwrap().is_empty());
    }
}
