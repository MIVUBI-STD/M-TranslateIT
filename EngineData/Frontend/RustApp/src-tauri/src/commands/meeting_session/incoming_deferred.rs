use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::engine::audio::live_segment_writer::remove_finalized_meeting_utterance_wav;

use super::{worker_blocker, worker_text};
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
        translation_style: String,
    },
    NeedsTranslation {
        transcript: String,
        source_language: String,
        target_language: String,
        translation_style: String,
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
    if let DeferredIncomingStage::NeedsAsr { audio_path, .. } = job.stage {
        remove_finalized_meeting_utterance_wav(&audio_path);
    }
}

pub(super) fn enqueue_deferred_incoming(job: DeferredIncomingJob) -> usize {
    let Ok(mut guard) = deferred_incoming_queue().lock() else {
        return 0;
    };
    guard.push_back(job);
    while guard.len() > MAX_DEFERRED_INCOMING {
        if let Some(evicted) = guard.pop_front() {
            cleanup_deferred_incoming_job(evicted);
        }
        DEFERRED_DROPPED_OVERFLOW.fetch_add(1, Ordering::Relaxed);
    }
    guard.len()
}

pub(super) fn requeue_deferred_incoming_front(job: DeferredIncomingJob) -> usize {
    let Ok(mut guard) = deferred_incoming_queue().lock() else {
        return 0;
    };
    guard.push_front(job);
    while guard.len() > MAX_DEFERRED_INCOMING {
        if let Some(evicted) = guard.pop_back() {
            cleanup_deferred_incoming_job(evicted);
        }
        DEFERRED_DROPPED_OVERFLOW.fetch_add(1, Ordering::Relaxed);
    }
    guard.len()
}

pub(super) fn take_due_deferred_incoming(
    session_id: &str,
    now_unix_ms: u64,
) -> Option<DeferredIncomingJob> {
    let mut guard = deferred_incoming_queue().lock().ok()?;
    while let Some(front) = guard.front() {
        if front.session_id != session_id {
            if let Some(stale) = guard.pop_front() {
                cleanup_deferred_incoming_job(stale);
            }
            DEFERRED_DROPPED_STALE.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        if now_unix_ms.saturating_sub(front.enqueued_unix_ms) > MAX_DEFERRED_INCOMING_AGE_MS {
            if let Some(stale) = guard.pop_front() {
                cleanup_deferred_incoming_job(stale);
            }
            DEFERRED_DROPPED_STALE.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        return guard.pop_front();
    }
    None
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

pub(super) fn clear_deferred_incoming_queue() -> Result<(), String> {
    let mut guard = deferred_incoming_queue()
        .lock()
        .map_err(|_| "meeting_incoming:deferred_queue_lock_failed".to_string())?;
    while let Some(job) = guard.pop_front() {
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
                translation_style: "natural".to_string(),
            },
            enqueued_unix_ms,
        }
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

    #[test]
    fn deferred_retry_preserves_first_deferral_age_budget() {
        clear_deferred_incoming_queue().expect("clear deferred queue");
        let first_deferred_unix_ms = 10_000;
        let retry_unix_ms = 29_000;
        let preserved = deferred_enqueue_unix_ms(Some(first_deferred_unix_ms), retry_unix_ms);
        assert_eq!(preserved, first_deferred_unix_ms);
        assert_eq!(deferred_enqueue_unix_ms(None, retry_unix_ms), retry_unix_ms);

        requeue_deferred_incoming_front(job("sess", 1, "t", preserved));
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

    #[test]
    fn deferred_queue_caps_evicts_expired_and_respects_session() {
        clear_deferred_incoming_queue().expect("clear deferred queue");
        assert!(deferred_incoming_queue().lock().unwrap().is_empty());

        enqueue_deferred_incoming(job("sess", 1, "t", 1_000));
        let stale_before = DEFERRED_DROPPED_STALE.fetch_and(0, Ordering::Relaxed);
        assert!(take_due_deferred_incoming("other-sess", 2_000).is_none());
        assert_eq!(
            DEFERRED_DROPPED_STALE.load(Ordering::Relaxed) - stale_before,
            1,
            "foreign-session job must be treated as stale"
        );

        for seq in 2..=7 {
            enqueue_deferred_incoming(job("sess", seq, "t", 10_000));
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

        enqueue_deferred_incoming(job("sess", 7, "a", 50_000));
        enqueue_deferred_incoming(job("sess", 8, "b", 51_000));
        let first = take_due_deferred_incoming("sess", 52_000).expect("fresh job due");
        let second = take_due_deferred_incoming("sess", 52_000).expect("second fresh job due");
        assert_eq!(first.event_sequence, 7);
        assert_eq!(second.event_sequence, 8);

        clear_deferred_incoming_queue().expect("clear deferred queue");
        assert!(deferred_incoming_queue().lock().unwrap().is_empty());
    }
}
