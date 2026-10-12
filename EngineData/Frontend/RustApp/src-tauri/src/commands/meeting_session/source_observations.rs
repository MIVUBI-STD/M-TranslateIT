//! Bounded ASR observation/stability candidate for Balanced Interpreter.
//! Only compiled by the Rust test harness until the canonical Meeting owner
//! can supply audio-epoch sample positions and authorize progressive commits.
//! Agreement across growing windows is TEXT evidence, not word/audio alignment.

const MAX_OBSERVATION_BYTES: usize = 8 * 1024;
const MAX_WINDOW_SAMPLES_16K: u64 = 16_000 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceLane {
    You,
    Incoming,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceScope {
    pub session_id: String,
    pub generation: Option<u64>,
    pub lane: SourceLane,
    pub speech_id: u64,
    pub audio_epoch: u64,
}

// End is exclusive. Coordinates are in canonical 16-kHz ASR input samples;
// a separate capture-to-ASR mapping is required before reporting microphone
// sample timestamps or phoneme-accurate boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AudioWindow {
    pub start_sample: u64,
    pub end_sample: u64,
}

#[derive(Debug, Clone)]
pub(super) struct AsrObservation {
    pub scope: SourceScope,
    pub revision: u64,
    pub window: AudioWindow,
    pub transcript: String,
    pub finalized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceSpanId {
    pub scope: SourceScope,
    pub window: AudioWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StabilityEvidence {
    // Text agrees on overlapping, anchored windows with NEW audio.
    // No part of that text has a known word-to-sample boundary.
    GrowingWindowAgreement,
    // Canonical source producer declared this complete audio window final.
    // This proves no alignment INSIDE the window.
    FinalizedWholeWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StableTextCandidate {
    pub text: String,
    pub revision: u64,
    pub evidence: StabilityEvidence,
    pub observation_window: AudioWindow,
    // Non-final agreement must never manufacture a committed audio span.
    pub whole_final_span: Option<SourceSpanId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ObservationOutcome {
    Pending,
    Stable(StableTextCandidate),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ObservationError {
    InvalidScope,
    ScopeMismatch,
    Closed,
    StaleRevision,
    InvalidWindow,
    RegressedWindow,
    InvalidTranscript,
}

pub(super) struct SourceStabilizer {
    scope: SourceScope,
    latest: Option<AsrObservation>,
    closed: bool,
}

impl SourceStabilizer {
    pub fn new(scope: SourceScope) -> Result<Self, ObservationError> {
        if scope.session_id.trim().is_empty()
            || scope.session_id.len() > 256
            || scope.speech_id == 0
            || !matches!(
                (scope.lane, scope.generation),
                (SourceLane::You, Some(_)) | (SourceLane::Incoming, None)
            )
        {
            return Err(ObservationError::InvalidScope);
        }
        Ok(Self { scope, latest: None, closed: false })
    }

    pub fn observe(&mut self, input: AsrObservation) -> Result<ObservationOutcome, ObservationError> {
        if self.closed {
            return Err(ObservationError::Closed);
        }
        if input.scope != self.scope {
            return Err(ObservationError::ScopeMismatch);
        }
        if input.revision == 0
            || self.latest.as_ref().is_some_and(|prev| input.revision <= prev.revision)
        {
            return Err(ObservationError::StaleRevision);
        }
        if input.window.end_sample <= input.window.start_sample
            || input.window.end_sample - input.window.start_sample > MAX_WINDOW_SAMPLES_16K
        {
            return Err(ObservationError::InvalidWindow);
        }
        if input.transcript.trim().is_empty()
            || input.transcript.len() > MAX_OBSERVATION_BYTES
            || input.transcript.contains('\0')
        {
            return Err(ObservationError::InvalidTranscript);
        }
        if self.latest.as_ref().is_some_and(|prev| {
            input.window.start_sample < prev.window.start_sample
                || input.window.end_sample < prev.window.end_sample
        }) {
            return Err(ObservationError::RegressedWindow);
        }

        let evidence = if input.finalized {
            // Finality must originate from the authoritative source producer,
            // not be inferred from punctuation or two matching ASR strings.
            let candidate = StableTextCandidate {
                text: input.transcript.trim().to_string(),
                revision: input.revision,
                evidence: StabilityEvidence::FinalizedWholeWindow,
                observation_window: input.window,
                whole_final_span: Some(SourceSpanId {
                    scope: self.scope.clone(),
                    window: input.window,
                }),
            };
            self.latest = Some(input);
            self.closed = true;
            return Ok(ObservationOutcome::Stable(candidate));
        } else if let Some(prev) = self.latest.as_ref() {
            if input.window.start_sample == prev.window.start_sample
                && input.window.end_sample > prev.window.end_sample
            {
                agreed_prefix(&prev.transcript, &input.transcript)
            } else {
                None
            }
        } else {
            None
        };

        let result = match evidence {
            Some(text) => ObservationOutcome::Stable(StableTextCandidate {
                text,
                revision: input.revision,
                evidence: StabilityEvidence::GrowingWindowAgreement,
                observation_window: input.window,
                whole_final_span: None,
            }),
            None => ObservationOutcome::Pending,
        };
        // One retained observation; new revisions replace the old hypothesis.
        // A shifted rolling window breaks text anchoring, so it cannot confirm
        // a prefix against the previous window.
        self.latest = Some(input);
        Ok(result)
    }
}

fn agreed_prefix(previous: &str, current: &str) -> Option<String> {
    let first: Vec<&str> = previous.split_whitespace().collect();
    let second: Vec<&str> = current.split_whitespace().collect();
    let shared = first.iter().zip(second.iter())
        .take_while(|(a, b)| a.to_lowercase() == b.to_lowercase())
        .count();
    // Intentionally retain punctuation and number/version distinctions:
    // e.g. "1.21" is NOT the same token as "1.21.1".
    if shared == 0 {
        None
    } else {
        Some(second[..shared].join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(epoch: u64) -> SourceScope {
        SourceScope {
            session_id: "s1".into(),
            generation: Some(7),
            lane: SourceLane::You,
            speech_id: 31,
            audio_epoch: epoch,
        }
    }

    fn observation(
        scope: &SourceScope, revision: u64, start: u64, end: u64,
        transcript: &str, finalized: bool,
    ) -> AsrObservation {
        AsrObservation {
            scope: scope.clone(), revision,
            window: AudioWindow { start_sample: start, end_sample: end },
            transcript: transcript.into(), finalized,
        }
    }

    #[test]
    fn growing_audio_confirms_text_only_not_fabricated_word_alignment() {
        let scope = scope(1);
        let mut tracker = SourceStabilizer::new(scope.clone()).unwrap();
        assert_eq!(
            tracker.observe(observation(&scope, 1, 0, 8_000, "Kita mulai", false)),
            Ok(ObservationOutcome::Pending),
        );
        let result = tracker.observe(observation(
            &scope, 2, 0, 14_000, "Kita mulai pengujian", false,
        )).unwrap();
        let ObservationOutcome::Stable(candidate) = result else {
            panic!("growing anchored audio should confirm a prefix");
        };
        assert_eq!(candidate.text, "Kita mulai");
        assert_eq!(candidate.evidence, StabilityEvidence::GrowingWindowAgreement);
        assert_eq!(candidate.whole_final_span, None);
        assert_eq!(candidate.revision, 2);
    }

    #[test]
    fn corrected_negation_and_version_are_not_confirmed() {
        let scope = scope(1);
        let mut tracker = SourceStabilizer::new(scope.clone()).unwrap();
        tracker.observe(observation(
            &scope, 1, 0, 6_000, "Kita tidak merilis 1.21", false,
        )).unwrap();
        let result = tracker.observe(observation(
            &scope, 2, 0, 12_000, "Kita akan merilis 1.21.1", false,
        )).unwrap();
        let ObservationOutcome::Stable(candidate) = result else { panic!("expected prefix") };
        assert_eq!(candidate.text, "Kita");
        let mut versions = SourceStabilizer::new(scope.clone()).unwrap();
        versions.observe(observation(
            &scope, 1, 0, 6_000, "Pakai versi 1.21", false,
        )).unwrap();
        let result = versions.observe(observation(
            &scope, 2, 0, 12_000, "Pakai versi 1.21.1", false,
        )).unwrap();
        let ObservationOutcome::Stable(candidate) = result else { panic!("expected prefix") };
        assert_eq!(candidate.text, "Pakai versi");
    }

    #[test]
    fn unchanged_audio_and_shifted_windows_cannot_fake_agreement() {
        let scope = scope(2);
        let mut tracker = SourceStabilizer::new(scope.clone()).unwrap();
        tracker.observe(observation(&scope, 1, 0, 8_000, "Server aktif", false)).unwrap();
        assert_eq!(
            tracker.observe(observation(&scope, 2, 0, 8_000, "Server aktif", false)),
            Ok(ObservationOutcome::Pending),
        );
        assert_eq!(
            tracker.observe(observation(&scope, 3, 4_000, 12_000, "Server aktif", false)),
            Ok(ObservationOutcome::Pending),
        );
        assert_eq!(
            tracker.observe(observation(&scope, 4, 4_000, 14_000, "Server aktif", false))
                .unwrap(),
            ObservationOutcome::Stable(StableTextCandidate {
                text: "Server aktif".into(),
                revision: 4,
                evidence: StabilityEvidence::GrowingWindowAgreement,
                observation_window: AudioWindow { start_sample: 4_000, end_sample: 14_000 },
                whole_final_span: None,
            }),
        );
    }

    #[test]
    fn final_window_closes_scope_and_rejects_foreign_or_stale_revision() {
        let scope = scope(3);
        let mut tracker = SourceStabilizer::new(scope.clone()).unwrap();
        assert_eq!(
            tracker.observe(observation(
                &SourceScope { audio_epoch: 4, ..scope.clone() },
                1, 0, 5_000, "salah", false,
            )),
            Err(ObservationError::ScopeMismatch),
        );
        tracker.observe(observation(&scope, 2, 0, 5_000, "Lanjut", false)).unwrap();
        assert_eq!(
            tracker.observe(observation(&scope, 1, 0, 6_000, "Lanjut", false)),
            Err(ObservationError::StaleRevision),
        );
        let candidate = tracker.observe(observation(
            &scope, 3, 0, 9_000, "Lanjutkan besok.", true,
        )).unwrap();
        let ObservationOutcome::Stable(candidate) = candidate else { panic!("final required") };
        assert_eq!(candidate.evidence, StabilityEvidence::FinalizedWholeWindow);
        assert_eq!(candidate.whole_final_span.unwrap().window.end_sample, 9_000);
        assert_eq!(
            tracker.observe(observation(&scope, 4, 0, 11_000, "ulang", true)),
            Err(ObservationError::Closed),
        );
    }
}
