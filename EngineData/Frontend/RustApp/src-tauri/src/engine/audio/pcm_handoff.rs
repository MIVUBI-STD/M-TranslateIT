//! Fixed-capacity PCM handoff prototype for one Meeting lane / session generation.
//! A future CPAL producer would never wait for a lock or allocate a chunk.
//! A loss invalidates buffered audio rather than joining speech across a gap.
//! NOT wired to production capture until offline playback/VAD replay and native
//! timing can be run; the legacy audio path remains the baseline.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, TryLockError};

/// Meeting capture direction. Stored once when the queue is constructed; never
/// read by the audio callback or inferred from a shared generation number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmLane {
    You,
    Incoming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmDropReason {
    Stopped,
    LockBusy,
    Poisoned,
    BufferFull,
    ChunkTooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmPush {
    Accepted(usize),
    Dropped(PcmDropReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PcmDrain {
    Samples(usize),
    Empty,
    Discontinuity,
    Stopped,
    Poisoned,
}

/// Numeric-only observations; no user audio or transcript content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmHandoffMetrics {
    pub generation: u64,
    pub capacity_samples: usize,
    pub buffered_samples: usize,
    pub dropped_callbacks: u64,
    pub dropped_samples: u64,
    pub discontinuity_epoch: u64,
    pub stopped: bool,
}

#[derive(Debug)]
struct PcmRing {
    data: Vec<f32>,
    head: usize,
    len: usize,
}

impl PcmRing {
    fn write(&mut self, samples: &[f32]) {
        let cap = self.data.len();
        let tail = (self.head + self.len) % cap;
        let first = samples.len().min(cap - tail);
        for (output, sample) in self.data[tail..tail + first]
            .iter_mut().zip(samples[..first].iter())
        {
            *output = safe_pcm(*sample);
        }
        for (output, sample) in self.data[..samples.len() - first]
            .iter_mut().zip(samples[first..].iter())
        {
            *output = safe_pcm(*sample);
        }
        self.len += samples.len();
    }

    fn drain_into(&mut self, output: &mut Vec<f32>, maximum: usize) -> usize {
        let count = self.len.min(maximum);
        let cap = self.data.len();
        let first = count.min(cap - self.head);
        output.clear();
        output.extend_from_slice(&self.data[self.head..self.head + first]);
        output.extend_from_slice(&self.data[..count - first]);
        self.head = (self.head + count) % cap;
        self.len -= count;
        count
    }

    fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    fn erase(&mut self) {
        self.data.fill(0.0);
        self.clear();
    }
}

fn safe_pcm(sample: f32) -> f32 {
    if sample.is_finite() {
        sample.clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/// One instance per authorized Meeting session, lane and generation. Never reuse after Stop.
/// A fixed preallocated ring prevents callback-side heap growth. The producer
/// uses try_lock; contention/overflow is accounted, not silently stitched.
pub struct PcmHandoff {
    session_id: String,
    lane: PcmLane,
    generation: u64,
    capacity_samples: usize,
    ring: Mutex<PcmRing>,
    buffered_samples: AtomicUsize,
    lost_callbacks: AtomicU64,
    lost_samples: AtomicU64,
    loss_epoch: AtomicU64,
    stopped: AtomicBool,
    consumer_claimed: AtomicBool,
}

impl PcmHandoff {
    pub fn new(
        session_id: &str,
        lane: PcmLane,
        generation: u64,
        capacity_samples: usize,
    ) -> Result<Self, &'static str> {
        if session_id.trim().is_empty()
            || generation == 0
            || !(1..=384_000).contains(&capacity_samples)
        {
            return Err("pcm_handoff:invalid_session_generation_or_capacity");
        }
        Ok(Self {
            session_id: session_id.to_owned(),
            lane,
            generation,
            capacity_samples,
            ring: Mutex::new(PcmRing {
                data: vec![0.0; capacity_samples],
                head: 0,
                len: 0,
            }),
            buffered_samples: AtomicUsize::new(0),
            lost_callbacks: AtomicU64::new(0),
            lost_samples: AtomicU64::new(0),
            loss_epoch: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
            consumer_claimed: AtomicBool::new(false),
        })
    }

    /// CPU-side identity gate. Callbacks only handle pre-bound PCM and do not
    /// allocate, compare session strings, or create a second session owner.
    pub fn belongs_to(&self, session_id: &str, lane: PcmLane, generation: u64) -> bool {
        !self.stopped.load(Ordering::Acquire)
            && self.session_id == session_id
            && self.lane == lane
            && self.generation == generation
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn lane(&self) -> PcmLane {
        self.lane
    }

    /// One CPU consumer per session/lane queue. A timed-out worker retains
    /// its claim so no new thread can consume its PCM concurrently.
    pub(super) fn reserve_consumer(&self) -> Result<(), &'static str> {
        if self.stopped.load(Ordering::Acquire) {
            return Err("pcm_consumer:already_stopped");
        }
        self.consumer_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "pcm_consumer:already_active")?;
        if self.stopped.load(Ordering::Acquire) {
            self.consumer_claimed.store(false, Ordering::Release);
            return Err("pcm_consumer:already_stopped");
        }
        Ok(())
    }

    /// Undo reservation only on OS thread spawn failure; never while live.
    pub(super) fn cancel_consumer_reservation(&self) {
        self.consumer_claimed.store(false, Ordering::Release);
    }

    fn record_loss(&self, samples: usize) {
        self.lost_callbacks.fetch_add(1, Ordering::Relaxed);
        self.lost_samples.fetch_add(samples as u64, Ordering::Relaxed);
        self.loss_epoch.fetch_add(1, Ordering::AcqRel);
    }

    /// Intended for CPAL. Does not wait for a contested mutex or grow storage.
    pub fn try_push(&self, samples: &[f32]) -> PcmPush {
        if self.stopped.load(Ordering::Acquire) {
            return PcmPush::Dropped(PcmDropReason::Stopped);
        }
        if samples.is_empty() {
            return PcmPush::Accepted(0);
        }
        if samples.len() > self.capacity_samples {
            self.record_loss(samples.len());
            return PcmPush::Dropped(PcmDropReason::ChunkTooLarge);
        }
        let mut ring = match self.ring.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => {
                self.record_loss(samples.len());
                return PcmPush::Dropped(PcmDropReason::LockBusy);
            }
            Err(TryLockError::Poisoned(_)) => {
                self.record_loss(samples.len());
                return PcmPush::Dropped(PcmDropReason::Poisoned);
            }
        };
        if self.stopped.load(Ordering::Acquire) {
            return PcmPush::Dropped(PcmDropReason::Stopped);
        }
        if ring.len.saturating_add(samples.len()) > self.capacity_samples {
            self.record_loss(samples.len());
            return PcmPush::Dropped(PcmDropReason::BufferFull);
        }
        ring.write(samples);
        self.buffered_samples.store(ring.len, Ordering::Release);
        PcmPush::Accepted(samples.len())
    }

    /// CPU worker call. `observed_loss_epoch` belongs to that worker and must
    /// persist across calls. Discontinuity discards ambiguous buffered samples
    /// and requires the VAD owner to reset its current phrase boundary.
    pub fn drain_into(
        &self,
        output: &mut Vec<f32>,
        maximum_samples: usize,
        observed_loss_epoch: &mut u64,
    ) -> PcmDrain {
        output.clear();
        if self.stopped.load(Ordering::Acquire) {
            return PcmDrain::Stopped;
        }
        let mut ring = match self.ring.lock() {
            Ok(guard) => guard,
            Err(_) => return PcmDrain::Poisoned,
        };
        if self.stopped.load(Ordering::Acquire) {
            return PcmDrain::Stopped;
        }
        let loss = self.loss_epoch.load(Ordering::Acquire);
        if loss != *observed_loss_epoch {
            // The old and new sides of an unavailable callback cannot form
            // one linguistic utterance. Fail closed, never invent continuity.
            ring.clear();
            self.buffered_samples.store(0, Ordering::Release);
            *observed_loss_epoch = loss;
            return PcmDrain::Discontinuity;
        }
        if ring.len == 0 || maximum_samples == 0 {
            return PcmDrain::Empty;
        }
        let count = ring.drain_into(output, maximum_samples);
        // Callback-side try_lock failure can publish a loss while this worker
        // holds the ring lock. Re-check before exposing drained audio to VAD.
        // Without this, a frame copied during loss can be mistaken for
        // uninterrupted speech on the receiving thread.
        let loss_after_copy = self.loss_epoch.load(Ordering::Acquire);
        if loss_after_copy != loss {
            ring.clear();
            self.buffered_samples.store(0, Ordering::Release);
            output.fill(0.0);
            output.clear();
            *observed_loss_epoch = loss_after_copy;
            return PcmDrain::Discontinuity;
        }
        self.buffered_samples.store(ring.len, Ordering::Release);
        PcmDrain::Samples(count)
    }

    /// Revoke admission immediately without waiting for the CPU drain mutex.
    /// The owner must stop the input stream before revocation, then join its
    /// consumer (with an explicit deadline) before buffer erasure/release.
    /// Revocation alone does not guarantee an in-flight VAD call has returned.
    pub fn request_stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }

    /// Erase retained samples after the worker has left. Existing test-only
    /// direct callers may use this without a separate consumer.
    pub fn stop(&self) {
        self.request_stop();
        // Poisoning does not mean retained private audio was erased.
        let mut ring = match self.ring.lock() {
            Ok(ring) => ring,
            Err(poisoned) => poisoned.into_inner(),
        };
        ring.erase();
        self.buffered_samples.store(0, Ordering::Release);
    }

    pub fn metrics(&self) -> PcmHandoffMetrics {
        PcmHandoffMetrics {
            generation: self.generation,
            capacity_samples: self.capacity_samples,
            buffered_samples: self.buffered_samples.load(Ordering::Acquire),
            dropped_callbacks: self.lost_callbacks.load(Ordering::Relaxed),
            dropped_samples: self.lost_samples.load(Ordering::Relaxed),
            discontinuity_epoch: self.loss_epoch.load(Ordering::Acquire),
            stopped: self.stopped.load(Ordering::Acquire),
        }
    }
}

/// CPU-only drain driver. The caller owns the VAD reset on a gap; no new
/// session or translation state is created here. Callbacks run after the
/// queue lock has been released.
pub struct PcmDrainCursor {
    loss_epoch: u64,
    samples: Vec<f32>,
}

impl PcmDrainCursor {
    pub fn new(max_chunk_samples: usize) -> Self {
        Self {
            loss_epoch: 0,
            samples: Vec::with_capacity(max_chunk_samples),
        }
    }

    pub fn pump(
        &mut self,
        handoff: &PcmHandoff,
        max_chunk_samples: usize,
        mut on_samples: impl FnMut(&[f32]),
        mut on_discontinuity: impl FnMut(),
    ) -> PcmDrain {
        let result = handoff.drain_into(
            &mut self.samples, max_chunk_samples, &mut self.loss_epoch,
        );
        match result {
            PcmDrain::Samples(_) => on_samples(&self.samples),
            // A poisoned ring has lost trustworthy PCM continuity as well:
            // revoke the VAD source phrase even though the CPU worker will
            // subsequently stop, rather than leaving an unclosed fragment.
            PcmDrain::Discontinuity | PcmDrain::Poisoned => on_discontinuity(),
            PcmDrain::Empty | PcmDrain::Stopped => {},
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::{PcmDrain, PcmDrainCursor, PcmDropReason, PcmHandoff, PcmLane, PcmPush};

    #[test]
    fn bounded_fifo_wraps_without_allocating_and_preserves_order() {
        let q = PcmHandoff::new("pcm-test", PcmLane::You, 11, 6).unwrap();
        let mut out = Vec::new();
        let mut seen = 0;
        assert_eq!(q.try_push(&[0.1, 0.2, 0.3, 0.4]), PcmPush::Accepted(4));
        assert_eq!(q.drain_into(&mut out, 3, &mut seen), PcmDrain::Samples(3));
        assert_eq!(out, vec![0.1, 0.2, 0.3]);
        assert_eq!(q.try_push(&[0.5, 0.6, 0.7, 0.8]), PcmPush::Accepted(4));
        assert_eq!(q.drain_into(&mut out, 6, &mut seen), PcmDrain::Samples(5));
        assert_eq!(out, vec![0.4, 0.5, 0.6, 0.7, 0.8]);
        assert_eq!(q.metrics().buffered_samples, 0);
        assert_eq!(q.metrics().capacity_samples, 6);
    }

    #[test]
    fn capacity_overflow_is_accounted_and_forces_phrase_discontinuity() {
        let q = PcmHandoff::new("pcm-test", PcmLane::You, 7, 4).unwrap();
        let mut out = Vec::new();
        let mut seen = 0;
        assert_eq!(q.try_push(&[0.1, 0.2, 0.3]), PcmPush::Accepted(3));
        assert_eq!(q.try_push(&[0.4, 0.5]), PcmPush::Dropped(PcmDropReason::BufferFull));
        assert_eq!(q.metrics().dropped_samples, 2);
        assert_eq!(q.metrics().discontinuity_epoch, 1);
        assert_eq!(q.drain_into(&mut out, 4, &mut seen), PcmDrain::Discontinuity);
        assert!(out.is_empty());
        assert_eq!(q.metrics().buffered_samples, 0);
        assert_eq!(q.try_push(&[0.7]), PcmPush::Accepted(1));
        assert_eq!(q.drain_into(&mut out, 4, &mut seen), PcmDrain::Samples(1));
        assert_eq!(out, vec![0.7]);
    }

    #[test]
    fn lock_contention_never_blocks_callback_and_invalidates_old_audio() {
        let q = PcmHandoff::new("pcm-test", PcmLane::You, 3, 4).unwrap();
        let mut seen = 0;
        let mut out = Vec::new();
        assert_eq!(q.try_push(&[0.2]), PcmPush::Accepted(1));
        let guard = q.ring.lock().unwrap();
        assert_eq!(q.try_push(&[0.3]), PcmPush::Dropped(PcmDropReason::LockBusy));
        drop(guard);
        assert_eq!(q.drain_into(&mut out, 4, &mut seen), PcmDrain::Discontinuity);
        assert_eq!(q.metrics().dropped_callbacks, 1);
    }

    #[test]
    fn invalid_float_and_oversize_chunks_never_poison_worker_output() {
        let q = PcmHandoff::new("pcm-test", PcmLane::You, 5, 4).unwrap();
        let mut out = Vec::new();
        let mut seen = 0;
        assert_eq!(q.try_push(&[f32::NAN, f32::INFINITY, 2.0, -2.0]), PcmPush::Accepted(4));
        assert_eq!(q.drain_into(&mut out, 4, &mut seen), PcmDrain::Samples(4));
        assert_eq!(out, vec![0.0, 0.0, 1.0, -1.0]);
        assert_eq!(q.try_push(&[0.0; 5]), PcmPush::Dropped(PcmDropReason::ChunkTooLarge));
        assert_eq!(q.drain_into(&mut out, 4, &mut seen), PcmDrain::Discontinuity);
    }

    #[test]
    fn cpu_driver_forwards_only_continuous_audio_and_reports_reset() {
        let handoff = PcmHandoff::new("pcm-test", PcmLane::You, 61, 4).unwrap();
        let mut cursor = PcmDrainCursor::new(4);
        let mut received = Vec::<f32>::new();
        let mut resets = 0;
        assert_eq!(handoff.try_push(&[0.1, 0.2]), PcmPush::Accepted(2));
        assert_eq!(
            cursor.pump(&handoff, 4, |audio| received.extend_from_slice(audio), || resets += 1),
            PcmDrain::Samples(2),
        );
        assert_eq!(received, vec![0.1, 0.2]);
        assert_eq!(resets, 0);
        assert_eq!(handoff.try_push(&[0.3; 5]), PcmPush::Dropped(PcmDropReason::ChunkTooLarge));
        assert_eq!(
            cursor.pump(&handoff, 4, |audio| received.extend_from_slice(audio), || resets += 1),
            PcmDrain::Discontinuity,
        );
        assert_eq!(resets, 1);
        assert_eq!(received, vec![0.1, 0.2]);
        assert_eq!(handoff.try_push(&[0.8]), PcmPush::Accepted(1));
        assert_eq!(
            cursor.pump(&handoff, 4, |audio| received.extend_from_slice(audio), || resets += 1),
            PcmDrain::Samples(1),
        );
        assert_eq!(received, vec![0.1, 0.2, 0.8]);
        handoff.stop();
        assert_eq!(
            cursor.pump(&handoff, 4, |_| panic!("stopped stream replayed"), || resets += 1),
            PcmDrain::Stopped,
        );
    }

    #[test]
    fn stop_discards_backlog_and_rejects_late_session_audio() {
        let q = PcmHandoff::new("pcm-test", PcmLane::You, 51, 8).unwrap();
        let mut out = Vec::new();
        let mut seen = 0;
        assert_eq!(q.try_push(&[0.25, 0.5]), PcmPush::Accepted(2));
        q.stop();
        assert!(q.ring.lock().unwrap().data.iter().all(|sample| *sample == 0.0));
        assert_eq!(q.try_push(&[0.75]), PcmPush::Dropped(PcmDropReason::Stopped));
        assert_eq!(q.drain_into(&mut out, 8, &mut seen), PcmDrain::Stopped);
        assert_eq!(q.metrics().buffered_samples, 0);
        assert_eq!(q.metrics().generation, 51);
        assert!(q.metrics().stopped);
        assert!(PcmHandoff::new("pcm-test", PcmLane::You, 0, 8).is_err());
        assert!(PcmHandoff::new("pcm-test", PcmLane::You, 1, 0).is_err());
    }

    #[test]
    fn session_lane_and_generation_must_match_before_replay() {
        let q = PcmHandoff::new("session-a", PcmLane::You, 42, 8).unwrap();
        assert!(q.belongs_to("session-a", PcmLane::You, 42));
        assert!(!q.belongs_to("session-b", PcmLane::You, 42));
        assert!(!q.belongs_to("session-a", PcmLane::Incoming, 42));
        assert!(!q.belongs_to("session-a", PcmLane::You, 43));
        assert!(PcmHandoff::new("", PcmLane::You, 42, 8).is_err());
        q.stop();
        assert!(!q.belongs_to("session-a", PcmLane::You, 42));
    }

    #[test]
    fn consumer_reservation_is_single_owner_until_spawn_fails_or_stop() {
        let q = PcmHandoff::new("pcm-claim", PcmLane::Incoming, 52, 8).unwrap();
        assert_eq!(q.reserve_consumer(), Ok(()));
        assert_eq!(q.reserve_consumer(), Err("pcm_consumer:already_active"));
        q.cancel_consumer_reservation();
        assert_eq!(q.reserve_consumer(), Ok(()));
        q.stop();
        assert_eq!(q.reserve_consumer(), Err("pcm_consumer:already_stopped"));
    }

    #[test]
    fn stop_scrubs_poisoned_audio_ring_instead_of_skipping_cleanup() {
        let q = PcmHandoff::new("pcm-poison", PcmLane::Incoming, 53, 8).unwrap();
        assert_eq!(q.try_push(&[0.5, 0.7]), PcmPush::Accepted(2));
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = q.ring.lock().unwrap();
            panic!("intentional mutex poison for Stop regression");
        }));
        assert!(poisoned.is_err());
        q.stop();
        let ring = q.ring.lock().unwrap_err().into_inner();
        assert!(ring.data.iter().all(|v| *v == 0.0));
        assert_eq!(q.metrics().buffered_samples, 0);
        assert_eq!(q.try_push(&[0.1]), PcmPush::Dropped(PcmDropReason::Stopped));
    }
    #[test]
    fn poisoned_pcm_drain_invalidates_vad_once_without_replaying_audio() {
        let q = PcmHandoff::new("pcm-poison-gap", PcmLane::Incoming, 54, 8).unwrap();
        assert_eq!(q.try_push(&[0.1, 0.2]), PcmPush::Accepted(2));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = q.ring.lock().unwrap();
            panic!("intentional poison of PCM ring");
        }));
        let mut cursor = PcmDrainCursor::new(8);
        let mut resets = 0;
        let result = cursor.pump(
            &q, 8, |_| panic!("poisoned audio was forwarded"), || resets += 1,
        );
        assert_eq!(result, PcmDrain::Poisoned);
        assert_eq!(resets, 1);
        q.stop();
    }

}
