//! Offline-only PCM → existing finalized-utterance VAD replay.
//!
//! This module deliberately does NOT create a second speech-boundary engine:
//! it forwards valid mono PCM to the existing Meeting finalized producer and
//! resets that producer on a dropped callback/overflow discontinuity.
//! NOT wired into CPAL, helper GPU work, or production Meeting session owners.

use super::finalized_utterance::{
    observe_finalized_incoming_mono_f32_samples, observe_finalized_outbound_mono_f32_samples,
    reset_finalized_incoming_speech_boundary, reset_finalized_outbound_speech_boundary,
};
use super::pcm_handoff::{PcmDrain, PcmDrainCursor, PcmHandoff, PcmLane};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

struct IncomingPcmVadReplay {
    session_id: String,
    cursor: PcmDrainCursor,
    max_samples_per_pump: usize,
    sample_rate_hz: u32,
    generation: u64,
    reset_count: u64,
}

impl IncomingPcmVadReplay {
    fn new(
        handoff: &PcmHandoff,
        sample_rate_hz: u32,
        max_samples_per_pump: usize,
    ) -> Result<Self, &'static str> {
        if handoff.lane() != PcmLane::Incoming
            || sample_rate_hz == 0
            || max_samples_per_pump == 0
            || max_samples_per_pump > handoff.metrics().capacity_samples
        {
            return Err("pcm_replay:invalid_worker_config");
        }
        Ok(Self {
            session_id: handoff.session_id().to_owned(),
            cursor: PcmDrainCursor::new(max_samples_per_pump),
            max_samples_per_pump,
            sample_rate_hz,
            generation: handoff.metrics().generation,
            reset_count: 0,
        })
    }

    fn pump(&mut self, handoff: &PcmHandoff) -> PcmDrain {
        if !handoff.belongs_to(&self.session_id, PcmLane::Incoming, self.generation) {
            return PcmDrain::Stopped;
        }
        let sample_rate_hz = self.sample_rate_hz;
        let mut reset_this_pump = false;
        let result = self.cursor.pump(
            handoff,
            self.max_samples_per_pump,
            |samples| {
                // Same canonical VAD as the legacy Meeting capture path.
                observe_finalized_incoming_mono_f32_samples(samples, sample_rate_hz);
            },
            || {
                reset_finalized_incoming_speech_boundary();
                reset_this_pump = true;
            },
        );
        if reset_this_pump {
            self.reset_count = self.reset_count.saturating_add(1);
        }
        result
    }
}

struct OutboundPcmVadReplay {
    session_id: String,
    cursor: PcmDrainCursor,
    max_samples_per_pump: usize,
    sample_rate_hz: u32,
    generation: u64,
}

impl OutboundPcmVadReplay {
    fn new(
        handoff: &PcmHandoff,
        sample_rate_hz: u32,
        max_samples_per_pump: usize,
    ) -> Result<Self, &'static str> {
        if handoff.lane() != PcmLane::You
            || sample_rate_hz == 0
            || max_samples_per_pump == 0
            || max_samples_per_pump > handoff.metrics().capacity_samples
        {
            return Err("pcm_replay:invalid_worker_config");
        }
        Ok(Self {
            session_id: handoff.session_id().to_owned(),
            cursor: PcmDrainCursor::new(max_samples_per_pump),
            max_samples_per_pump,
            sample_rate_hz,
            generation: handoff.metrics().generation,
        })
    }

    fn pump(&mut self, handoff: &PcmHandoff) -> PcmDrain {
        if !handoff.belongs_to(&self.session_id, PcmLane::You, self.generation) {
            return PcmDrain::Stopped;
        }
        let sample_rate_hz = self.sample_rate_hz;
        let generation = self.generation;
        self.cursor.pump(
            handoff,
            self.max_samples_per_pump,
            |samples| observe_finalized_outbound_mono_f32_samples(samples, sample_rate_hz),
            || reset_finalized_outbound_speech_boundary(generation),
        )
    }
}

// Source-only CPU consumer seam. No CPAL capture or production Meeting owner
// uses this yet. Reuse the same canonical VAD replay types; do not spawn a
// second capture/segmentation engine or run inference on this thread.
enum CpuVadReplay {
    Incoming(IncomingPcmVadReplay),
    Outbound(OutboundPcmVadReplay),
}

impl CpuVadReplay {
    fn pump(&mut self, handoff: &PcmHandoff) -> PcmDrain {
        match self {
            Self::Incoming(replay) => replay.pump(handoff),
            Self::Outbound(replay) => replay.pump(handoff),
        }
    }
}

struct PcmCpuConsumer {
    handoff: Arc<PcmHandoff>,
    thread: Option<JoinHandle<()>>,
}

impl PcmCpuConsumer {
    fn start(
        handoff: Arc<PcmHandoff>,
        sample_rate_hz: u32,
        max_samples_per_pump: usize,
    ) -> Result<Self, &'static str> {
        Self::start_with_hook(handoff, sample_rate_hz, max_samples_per_pump, || {})
    }

    // The one-shot hook is used only to reproduce a stalled CPU worker in
    // deterministic source tests. It is never invoked by the audio callback.
    fn start_with_hook(
        handoff: Arc<PcmHandoff>,
        sample_rate_hz: u32,
        max_samples_per_pump: usize,
        before_first_pump: impl FnOnce() + Send + 'static,
    ) -> Result<Self, &'static str> {
        if handoff.metrics().stopped {
            return Err("pcm_consumer:already_stopped");
        }
        let replay = match handoff.lane() {
            PcmLane::Incoming => CpuVadReplay::Incoming(IncomingPcmVadReplay::new(
                &handoff, sample_rate_hz, max_samples_per_pump,
            )?),
            PcmLane::You => CpuVadReplay::Outbound(OutboundPcmVadReplay::new(
                &handoff, sample_rate_hz, max_samples_per_pump,
            )?),
        };
        handoff.reserve_consumer()?;
        let for_worker = Arc::clone(&handoff);
        let handle = thread::Builder::new()
            .name("translateit-pcm-cpu-replay".to_owned())
            .spawn(move || {
                let mut replay = replay;
                before_first_pump();
                loop {
                    if for_worker.metrics().stopped {
                        break;
                    }
                    match replay.pump(&for_worker) {
                        PcmDrain::Samples(_) | PcmDrain::Discontinuity => {}
                        // Bounded idle poll; the real-time producer only pushes
                        // PCM and does not perform OS wakeups or extra locking.
                        PcmDrain::Empty => thread::park_timeout(Duration::from_millis(5)),
                        PcmDrain::Stopped | PcmDrain::Poisoned => break,
                    }
                }
            });
        let handle = match handle {
            Ok(handle) => handle,
            Err(_) => {
                handoff.cancel_consumer_reservation();
                return Err("pcm_consumer:spawn_failed");
            }
        };
        Ok(Self { handoff, thread: Some(handle) })
    }

    /// Revoke first, wake the idle worker, and only join if it actually exited.
    /// A timeout keeps the handle owned for a later retry; it is not a
    /// successful Stop and must never authorize production capture cutover.
    fn stop(&mut self, timeout: Duration) -> Result<(), &'static str> {
        self.handoff.request_stop();
        if let Some(handle) = self.thread.as_ref() {
            handle.thread().unpark();
            let deadline = Instant::now()
                .checked_add(timeout)
                .ok_or("pcm_consumer:invalid_stop_deadline")?;
            while !handle.is_finished() {
                if Instant::now() >= deadline {
                    return Err("pcm_consumer:stop_deadline_exceeded");
                }
                thread::sleep(Duration::from_millis(1));
            }
        }
        if let Some(handle) = self.thread.take() {
            let joined = handle.join().is_ok();
            self.handoff.stop();
            if !joined {
                return Err("pcm_consumer:worker_panicked");
            }
        } else {
            self.handoff.stop();
        }
        Ok(())
    }
}

impl Drop for PcmCpuConsumer {
    fn drop(&mut self) {
        // Drop must not block the caller. A previous Stop timeout means the
        // worker may still be active; this is not a production-safe fallback.
        self.handoff.request_stop();
        if let Some(handle) = self.thread.as_ref() {
            handle.thread().unpark();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IncomingPcmVadReplay, OutboundPcmVadReplay, PcmCpuConsumer, PcmDrain, PcmHandoff, PcmLane};
    use std::sync::{mpsc, Arc};
    use std::thread;
    use std::time::Duration;
    use crate::engine::audio::finalized_utterance::{
        clear_finalized_incoming_utterance_producer, clear_finalized_outbound_utterance_producer,
        clear_finalized_meeting_sequence, reset_finalized_incoming_utterance_producer,
        reset_finalized_outbound_utterance_producer, reset_finalized_meeting_sequence,
        try_take_finalized_incoming_utterance, wait_take_finalized_outbound_utterance,
    };

    struct ReplayFixture {
        handoff: PcmHandoff,
        worker: IncomingPcmVadReplay,
        session_id: &'static str,
    }

    impl ReplayFixture {
        fn new(session_id: &'static str, generation: u64, capacity: usize) -> Self {
            reset_finalized_meeting_sequence(session_id);
            reset_finalized_incoming_utterance_producer(session_id, 16_000);
            let handoff = PcmHandoff::new(session_id, PcmLane::Incoming, generation, capacity).expect("queue creation");
            let worker = IncomingPcmVadReplay::new(&handoff, 16_000, capacity)
                .expect("worker configuration");
            Self { handoff, worker, session_id }
        }

        fn send_and_pump(&mut self, samples: &[f32]) -> PcmDrain {
            assert!(matches!(
                self.handoff.try_push(samples),
                super::super::pcm_handoff::PcmPush::Accepted(_),
            ));
            self.worker.pump(&self.handoff)
        }
    }

    impl Drop for ReplayFixture {
        fn drop(&mut self) {
            self.handoff.stop();
            clear_finalized_incoming_utterance_producer();
            clear_finalized_meeting_sequence();
        }
    }

    #[test]
    fn canonical_vad_receives_replayed_speech_without_new_segmenter() {
        let _guard = crate::engine::runtime_state::runtime_test_guard();
        let mut fixture = ReplayFixture::new("pcm-replay-natural", 10, 16_000);
        assert_eq!(fixture.send_and_pump(&[0.10; 6_400]), PcmDrain::Samples(6_400));
        assert_eq!(fixture.send_and_pump(&[0.0; 6_400]), PcmDrain::Samples(6_400));
        let finalized = try_take_finalized_incoming_utterance(fixture.session_id)
            .expect("existing VAD should finalize a natural speech + silence turn");
        assert_eq!(finalized.session_id, fixture.session_id);
        assert_eq!(finalized.lane, "incoming");
        assert_eq!(finalized.frame.sample_rate_hz, 16_000);
        assert_eq!(finalized.frame.channels, 1);
        assert!(finalized.frame.samples.iter().all(|sample| sample.is_finite()));
        assert!(try_take_finalized_incoming_utterance(fixture.session_id).is_none());
        assert_eq!(fixture.worker.reset_count, 0);
    }

    #[test]
    fn dropped_chunk_resets_existing_vad_instead_of_stitching_speech() {
        let _guard = crate::engine::runtime_state::runtime_test_guard();
        let mut fixture = ReplayFixture::new("pcm-replay-gap", 11, 6_400);
        assert_eq!(fixture.send_and_pump(&[0.10; 6_400]), PcmDrain::Samples(6_400));
        assert!(matches!(
            fixture.handoff.try_push(&[0.10; 6_401]),
            super::super::pcm_handoff::PcmPush::Dropped(
                super::super::pcm_handoff::PcmDropReason::ChunkTooLarge
            ),
        ));
        assert_eq!(fixture.worker.pump(&fixture.handoff), PcmDrain::Discontinuity);
        assert_eq!(fixture.worker.reset_count, 1);
        assert_eq!(fixture.send_and_pump(&[0.0; 6_400]), PcmDrain::Samples(6_400));
        assert!(try_take_finalized_incoming_utterance(fixture.session_id).is_none());
        // Fresh post-gap utterance has its own recognized phrase boundary.
        assert_eq!(fixture.send_and_pump(&[0.10; 6_400]), PcmDrain::Samples(6_400));
        assert_eq!(fixture.send_and_pump(&[0.0; 6_400]), PcmDrain::Samples(6_400));
        assert!(try_take_finalized_incoming_utterance(fixture.session_id).is_some());
    }

    #[test]
    fn canonical_outbound_vad_runs_on_cpu_replay_without_synthesizing_audio() {
        use crate::engine::runtime_state::{
            begin_application_meeting_session, clear_runtime_session_state,
        };
        let _guard = crate::engine::runtime_state::runtime_test_guard();
        let _ = clear_runtime_session_state();
        let start = begin_application_meeting_session();
        assert!(start.blocker.is_empty(), "{}", start.blocker);
        let session = start.snapshot.expect("Meeting generation");
        let generation = session.generation;
        reset_finalized_meeting_sequence(&session.session_id);
        reset_finalized_outbound_utterance_producer(&session.session_id, generation, 16_000);

        let q = PcmHandoff::new(&session.session_id, PcmLane::You, generation, 16_000).unwrap();
        let mut worker = OutboundPcmVadReplay::new(&q, 16_000, 16_000).unwrap();
        assert_eq!(q.try_push(&[0.10; 6_400]), super::super::pcm_handoff::PcmPush::Accepted(6_400));
        assert_eq!(worker.pump(&q), PcmDrain::Samples(6_400));
        assert_eq!(q.try_push(&[0.0; 6_400]), super::super::pcm_handoff::PcmPush::Accepted(6_400));
        assert_eq!(worker.pump(&q), PcmDrain::Samples(6_400));
        let phrase = wait_take_finalized_outbound_utterance(generation)
            .expect("current authoritative Meeting VAD finalized a speech turn");
        assert_eq!(phrase.session_id, session.session_id);
        assert_eq!(phrase.generation, Some(generation));
        assert_eq!(phrase.lane, "you");
        assert_eq!(phrase.frame.sample_rate_hz, 16_000);
        assert!(phrase.speech_duration_ms >= 400);

        q.stop();
        assert_eq!(worker.pump(&q), PcmDrain::Stopped);
        clear_finalized_outbound_utterance_producer();
        clear_finalized_meeting_sequence();
        let _ = clear_runtime_session_state();
    }

    #[test]
    fn stale_generation_and_stopped_queue_never_reach_vad() {
        let _guard = crate::engine::runtime_state::runtime_test_guard();
        let mut fixture = ReplayFixture::new("pcm-replay-stopped", 12, 8_000);
        assert!(IncomingPcmVadReplay::new(&fixture.handoff, 0, 1).is_err());
        assert!(IncomingPcmVadReplay::new(&fixture.handoff, 16_000, 0).is_err());
        assert!(IncomingPcmVadReplay::new(&fixture.handoff, 16_000, 8_001).is_err());
        let foreign_session = PcmHandoff::new(
            "another-session", PcmLane::Incoming, 12, 8_000,
        ).unwrap();
        assert_eq!(fixture.worker.pump(&foreign_session), PcmDrain::Stopped);
        let foreign_lane = PcmHandoff::new(
            fixture.session_id, PcmLane::You, 12, 8_000,
        ).unwrap();
        assert_eq!(fixture.worker.pump(&foreign_lane), PcmDrain::Stopped);
        let foreign_generation = PcmHandoff::new(
            fixture.session_id, PcmLane::Incoming, 13, 8_000,
        ).unwrap();
        assert_eq!(fixture.worker.pump(&foreign_generation), PcmDrain::Stopped);
        assert!(IncomingPcmVadReplay::new(&foreign_lane, 16_000, 8_000).is_err());
        assert!(try_take_finalized_incoming_utterance(fixture.session_id).is_none());
        fixture.handoff.stop();
        assert_eq!(fixture.worker.pump(&fixture.handoff), PcmDrain::Stopped);
        assert!(try_take_finalized_incoming_utterance(fixture.session_id).is_none());
    }

    #[test]
    fn cpu_consumer_forwards_to_canonical_vad_and_joins_after_stop() {
        let _guard = crate::engine::runtime_state::runtime_test_guard();
        let session_id = "pcm-cpu-replay-incoming";
        reset_finalized_meeting_sequence(session_id);
        reset_finalized_incoming_utterance_producer(session_id, 16_000);
        let handoff = Arc::new(
            PcmHandoff::new(session_id, PcmLane::Incoming, 31, 16_000).unwrap(),
        );
        let mut worker = PcmCpuConsumer::start(Arc::clone(&handoff), 16_000, 6_400)
            .expect("start canonical VAD consumer");
        assert_eq!(
            handoff.try_push(&[0.10; 6_400]),
            super::super::pcm_handoff::PcmPush::Accepted(6_400),
        );
        assert_eq!(
            handoff.try_push(&[0.0; 6_400]),
            super::super::pcm_handoff::PcmPush::Accepted(6_400),
        );

        let mut recognized = None;
        for _ in 0..200 {
            recognized = try_take_finalized_incoming_utterance(session_id);
            if recognized.is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let recognized = recognized.expect("CPU replay must drive canonical VAD");
        assert_eq!(recognized.session_id, session_id);
        assert_eq!(recognized.lane, "incoming");
        assert!(worker.stop(Duration::from_secs(1)).is_ok());
        assert_eq!(handoff.metrics().buffered_samples, 0);
        assert!(handoff.metrics().stopped);
        assert_eq!(
            handoff.try_push(&[0.20]),
            super::super::pcm_handoff::PcmPush::Dropped(
                super::super::pcm_handoff::PcmDropReason::Stopped
            ),
        );
        assert!(try_take_finalized_incoming_utterance(session_id).is_none());
        clear_finalized_incoming_utterance_producer();
        clear_finalized_meeting_sequence();
    }

    #[test]
    fn stalled_cpu_worker_timeout_is_reported_and_retry_joins_without_stale_audio() {
        let _guard = crate::engine::runtime_state::runtime_test_guard();
        let session_id = "pcm-cpu-replay-stall";
        reset_finalized_meeting_sequence(session_id);
        reset_finalized_incoming_utterance_producer(session_id, 16_000);
        let handoff = Arc::new(
            PcmHandoff::new(session_id, PcmLane::Incoming, 32, 16_000).unwrap(),
        );
        let (started_tx, started_rx) = mpsc::sync_channel::<()>(1);
        let (release_tx, release_rx) = mpsc::sync_channel::<()>(1);
        let mut worker = PcmCpuConsumer::start_with_hook(
            Arc::clone(&handoff), 16_000, 6_400, move || {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            },
        ).unwrap();
        started_rx.recv_timeout(Duration::from_secs(1)).expect("worker reached hook");
        assert!(matches!(
            PcmCpuConsumer::start(Arc::clone(&handoff), 16_000, 6_400),
            Err("pcm_consumer:already_active")
        ));
        assert_eq!(
            handoff.try_push(&[0.10; 6_400]),
            super::super::pcm_handoff::PcmPush::Accepted(6_400),
        );
        assert_eq!(
            worker.stop(Duration::from_millis(5)),
            Err("pcm_consumer:stop_deadline_exceeded"),
        );
        assert!(handoff.metrics().stopped);
        assert!(matches!(
            PcmCpuConsumer::start(Arc::clone(&handoff), 16_000, 6_400),
            Err("pcm_consumer:already_stopped")
        ));
        assert_eq!(
            handoff.try_push(&[0.20]),
            super::super::pcm_handoff::PcmPush::Dropped(
                super::super::pcm_handoff::PcmDropReason::Stopped
            ),
        );
        release_tx.send(()).expect("release controlled worker stall");
        worker.stop(Duration::from_secs(1)).expect("reap revoked CPU worker");
        assert_eq!(handoff.metrics().buffered_samples, 0);
        assert!(try_take_finalized_incoming_utterance(session_id).is_none());
        clear_finalized_incoming_utterance_producer();
        clear_finalized_meeting_sequence();
    }
}
