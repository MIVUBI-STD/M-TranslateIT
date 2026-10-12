//! Bounded ASR observation/stability candidate for Balanced Interpreter.
//! Only compiled by the Rust test harness until the canonical Meeting owner
//! can supply audio-epoch sample positions and authorize progressive commits.
//! Agreement across growing windows is TEXT evidence, not word/audio alignment.

const ASR_RATE_HZ: u32 = 16_000;
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

// Producer-supplied provenance for ONE continuous, untrimmed resampling run.
// The map is not synthesized from a transcript. A PCM gap, rate change,
// VAD removal, or resampler reset must start a new epoch/map.
#[derive(Debug, Clone)]
pub(super) struct CaptureClockSegment {
    pub scope: SourceScope,
    pub asr_window: AudioWindow,
    pub capture_window: AudioWindow,
    pub capture_rate_hz: u32,
}

// Outward-rounded capture sample bounds derived from the two clock rates.
// These are approximate bounds for the WHOLE audio window, not word timestamps
// and not independent proof that the producer preserved the sample content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CaptureWindowBounds {
    pub start_lower_sample: u64,
    pub end_upper_sample: u64,
    pub sample_rate_hz: u32,
}

impl CaptureClockSegment {
    pub fn map_window(
        &self,
        scope: &SourceScope,
        window: AudioWindow,
    ) -> Result<CaptureWindowBounds, ObservationError> {
        if &self.scope != scope {
            return Err(ObservationError::ScopeMismatch);
        }
        if !(8_000..=192_000).contains(&self.capture_rate_hz)
            || self.asr_window.end_sample <= self.asr_window.start_sample
            || self.capture_window.end_sample <= self.capture_window.start_sample
            || self.asr_window.end_sample - self.asr_window.start_sample > MAX_WINDOW_SAMPLES_16K
        {
            return Err(ObservationError::InvalidMapping);
        }
        let asr_count = u128::from(self.asr_window.end_sample - self.asr_window.start_sample);
        let capture_count = u128::from(self.capture_window.end_sample - self.capture_window.start_sample);
        let ratio_error = (asr_count * u128::from(self.capture_rate_hz))
            .abs_diff(capture_count * u128::from(ASR_RATE_HZ));
        // Up to one sample in either domain for roundoff at a resampler edge.
        if ratio_error > u128::from(self.capture_rate_hz) + u128::from(ASR_RATE_HZ) {
            return Err(ObservationError::InvalidMapping);
        }
        if window.start_sample < self.asr_window.start_sample
            || window.end_sample > self.asr_window.end_sample
            || window.end_sample <= window.start_sample
        {
            return Err(ObservationError::UnmappedWindow);
        }
        let rate = u128::from(self.capture_rate_hz);
        let asr_rate = u128::from(ASR_RATE_HZ);
        let start_delta = u128::from(window.start_sample - self.asr_window.start_sample) * rate / asr_rate;
        let end_numerator = u128::from(window.end_sample - self.asr_window.start_sample) * rate;
        let end_delta = end_numerator.div_ceil(asr_rate);
        let cap_start = u128::from(self.capture_window.start_sample);
        let cap_end = u128::from(self.capture_window.end_sample);
        let lower = cap_start + start_delta;
        let upper = (cap_start + end_delta).min(cap_end);
        if lower >= upper || upper > u128::from(u64::MAX) {
            return Err(ObservationError::InvalidMapping);
        }
        Ok(CaptureWindowBounds {
            start_lower_sample: lower as u64,
            end_upper_sample: upper as u64,
            sample_rate_hz: self.capture_rate_hz,
        })
    }
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
    pub asr_window: AudioWindow,
    // Only present when caller supplied matching capture clock provenance.
    pub capture_bounds: CaptureWindowBounds,
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
    // Entire ASR audio window can be bounded in capture coordinates, even
    // though no individual WORD in the stable text is time-aligned.
    pub capture_window_bounds: Option<CaptureWindowBounds>,
    // Non-final agreement or a final hypothesis without capture mapping
    // must never manufacture a committed capture source span.
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
    DiscontinuousWindow,
    InvalidTranscript,
    InvalidMapping,
    UnmappedWindow,
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
        self.observe_with_capture_mapping(input, None)
    }

    pub fn observe_with_capture_mapping(
        &mut self,
        input: AsrObservation,
        capture_map: Option<&CaptureClockSegment>,
    ) -> Result<ObservationOutcome, ObservationError> {
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
        if self.latest.as_ref().is_some_and(|prev| {
            input.window.start_sample > prev.window.start_sample
                && input.window.start_sample >= prev.window.end_sample
        }) {
            // A gap between rolling windows requires a new speech scope
            // or explicit finalization. Do not bridge it with text agreement.
            return Err(ObservationError::DiscontinuousWindow);
        }
        // Bad capture provenance cannot advance the revision or replace the
        // previous text hypothesis. The map is optional; missing is UNKNOWN.
        let mapped_bounds = capture_map
            .map(|mapping| mapping.map_window(&self.scope, input.window))
            .transpose()?;

        let evidence = if input.finalized {
            // Finality must originate from the authoritative source producer,
            // not be inferred from punctuation or two matching ASR strings.
            let candidate = StableTextCandidate {
                text: input.transcript.trim().to_string(),
                revision: input.revision,
                evidence: StabilityEvidence::FinalizedWholeWindow,
                observation_window: input.window,
                capture_window_bounds: mapped_bounds,
                whole_final_span: mapped_bounds.map(|capture_bounds| SourceSpanId {
                    scope: self.scope.clone(),
                    asr_window: input.window,
                    capture_bounds,
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
                capture_window_bounds: mapped_bounds,
                whole_final_span: None,
            }),
            None => ObservationOutcome::Pending,
        };
        // One retained observation; new revisions replace the old hypothesis.
        // On a shifted, overlapping rolling window, retire the previous text
        // anchor and build fresh agreement only from later growing windows
        // at the new start. Without word timing, suffix-prefix matching alone
        // could mistake repeated phrases for a proven audio anchor.
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
                capture_window_bounds: None,
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
        // No real capture-clock provenance was supplied: final ASR text is
        // stable, but source capture bounds must stay unknown.
        assert_eq!(candidate.whole_final_span, None);
        assert_eq!(
            tracker.observe(observation(&scope, 4, 0, 11_000, "ulang", true)),
            Err(ObservationError::Closed),
        );
    }
    fn map(scope: &SourceScope, asr_start: u64, asr_end: u64, capture_start: u64, capture_end: u64, rate: u32) -> CaptureClockSegment {
        CaptureClockSegment {
            scope: scope.clone(),
            asr_window: AudioWindow { start_sample: asr_start, end_sample: asr_end },
            capture_window: AudioWindow { start_sample: capture_start, end_sample: capture_end },
            capture_rate_hz: rate,
        }
    }

    #[test]
    fn capture_clock_maps_shifted_whole_windows_without_word_timing() {
        let scope = scope(5);
        let mut tracker = SourceStabilizer::new(scope.clone()).unwrap();
        let first = map(&scope, 0, 8_000, 0, 24_000, 48_000);
        assert_eq!(
            tracker.observe_with_capture_mapping(
                observation(&scope, 1, 0, 8_000, "Server aktif", false), Some(&first)),
            Ok(ObservationOutcome::Pending),
        );
        let shifted = map(&scope, 4_000, 12_000, 12_000, 36_000, 48_000);
        assert_eq!(
            tracker.observe_with_capture_mapping(
                observation(&scope, 2, 4_000, 12_000, "Server aktif", false), Some(&shifted)),
            Ok(ObservationOutcome::Pending),
        );
        let grown = map(&scope, 4_000, 14_000, 12_000, 42_000, 48_000);
        let outcome = tracker.observe_with_capture_mapping(
            observation(&scope, 3, 4_000, 14_000, "Server aktif besok", false), Some(&grown),
        ).unwrap();
        let ObservationOutcome::Stable(candidate) = outcome else { panic!("stable prefix") };
        assert_eq!(candidate.text, "Server aktif");
        assert_eq!(candidate.capture_window_bounds, Some(CaptureWindowBounds {
            start_lower_sample: 12_000,
            end_upper_sample: 42_000,
            sample_rate_hz: 48_000,
        }));
        assert_eq!(candidate.whole_final_span, None);
        let window = grown.map_window(&scope, AudioWindow {
            start_sample: 6_000, end_sample: 10_000,
        }).unwrap();
        assert_eq!(window.start_lower_sample, 18_000);
        assert_eq!(window.end_upper_sample, 30_000);
    }

    #[test]
    fn final_audio_span_needs_matching_real_capture_clock_provenance() {
        let scope = scope(6);
        let mut tracker = SourceStabilizer::new(scope.clone()).unwrap();
        let clock = map(&scope, 0, 16_000, 24_000, 68_100, 44_100);
        let outcome = tracker.observe_with_capture_mapping(
            observation(&scope, 1, 0, 16_000, "Kita mulai besok.", true), Some(&clock),
        ).unwrap();
        let ObservationOutcome::Stable(candidate) = outcome else { panic!("final") };
        let span = candidate.whole_final_span.expect("capture clock needed");
        assert_eq!(span.asr_window.end_sample, 16_000);
        assert_eq!(span.capture_bounds, CaptureWindowBounds {
            start_lower_sample: 24_000, end_upper_sample: 68_100,
            sample_rate_hz: 44_100,
        });
    }

    #[test]
    fn mismatched_clock_or_gap_cannot_advance_asr_revision() {
        let scope = scope(7);
        let mut tracker = SourceStabilizer::new(scope.clone()).unwrap();
        let wrong_scope = map(&SourceScope { audio_epoch: 8, ..scope.clone() }, 0, 8_000, 0, 24_000, 48_000);
        assert_eq!(
            tracker.observe_with_capture_mapping(
                observation(&scope, 1, 0, 8_000, "Mulai", false), Some(&wrong_scope)),
            Err(ObservationError::ScopeMismatch),
        );
        let bad_ratio = map(&scope, 0, 8_000, 0, 30_000, 48_000);
        assert_eq!(
            tracker.observe_with_capture_mapping(
                observation(&scope, 1, 0, 8_000, "Mulai", false), Some(&bad_ratio)),
            Err(ObservationError::InvalidMapping),
        );
        let good = map(&scope, 0, 8_000, 0, 24_000, 48_000);
        assert_eq!(
            tracker.observe_with_capture_mapping(
                observation(&scope, 1, 0, 8_000, "Mulai", false), Some(&good)),
            Ok(ObservationOutcome::Pending),
        );
        assert_eq!(
            tracker.observe(observation(&scope, 2, 8_000, 16_000, "Lanjut", false)),
            Err(ObservationError::DiscontinuousWindow),
        );
        assert_eq!(
            tracker.observe(observation(&scope, 2, 0, 12_000, "Mulai lagi", false))
                .unwrap(),
            ObservationOutcome::Stable(StableTextCandidate {
                text: "Mulai".into(),
                revision: 2,
                evidence: StabilityEvidence::GrowingWindowAgreement,
                observation_window: AudioWindow { start_sample: 0, end_sample: 12_000 },
                capture_window_bounds: None,
                whole_final_span: None,
            }),
        );
    }

}
