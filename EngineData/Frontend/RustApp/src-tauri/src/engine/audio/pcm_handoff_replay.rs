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

#[cfg(test)]
mod tests {
    use super::{IncomingPcmVadReplay, OutboundPcmVadReplay, PcmDrain, PcmHandoff, PcmLane};
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
}
