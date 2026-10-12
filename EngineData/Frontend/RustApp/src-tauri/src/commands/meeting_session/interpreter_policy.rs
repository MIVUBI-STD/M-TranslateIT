//! Pure decision boundary for Balanced Interpreter source eligibility.
//! Test-only: PR-032 still requires finalized utterances in production.
//! This module NEVER commits a source, requests MiLMMT, or authorizes TTS.
//! A matching Meeting owner must still atomically claim each ready source.

use super::source_observations::{
    SourceScope, SourceSpanId, StabilityEvidence, StableTextCandidate,
};

const MAX_CANDIDATE_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundaryRisk {
    TrailingNegation,
    OpenConditional,
    PossibleRepair,
    OpenConnector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WaitReason {
    InvalidAuthority,
    StaleRevision,
    InvalidText,
    OpenMeaning(BoundaryRisk),
    NoAlignedPartialBoundary,
    MissingFinalSpan,
    InconsistentFinalSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CommitCandidate {
    pub scope: SourceScope,
    pub revision: u64,
    pub source_span: SourceSpanId,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PolicyDecision {
    Wait(WaitReason),
    CommitReady(CommitCandidate),
}

/// A bounded advisory hint, NOT a semantic correctness classifier.
/// Natural-language syntax is too variable for punctuation/suffix checks
/// to assert that a translation is complete or faithful.
pub(super) fn boundary_risk(text: &str) -> Option<BoundaryRisk> {
    let lower = text.trim().to_lowercase();
    let normalized = lower.trim_end_matches(|c: char| c.is_whitespace()
        || matches!(c, ',' | '.' | '!' | '?' | ';' | ':' | '…'));
    let last = normalized.split_whitespace().last().unwrap_or_default();

    if last == "eh"
        || normalized.ends_with("maksud saya")
        || normalized.ends_with("lebih tepatnya")
    {
        return Some(BoundaryRisk::PossibleRepair);
    }
    if matches!(last, "tidak" | "belum" | "bukan" | "jangan") {
        return Some(BoundaryRisk::TrailingNegation);
    }
    if matches!(last, "kalau" | "jika" | "apabila" | "karena" | "sehingga")
        || (["kalau ", "jika ", "apabila "].iter().any(|p| normalized.starts_with(p))
            && !normalized.contains(',')
            && !normalized.split_whitespace().any(|w| w == "maka"))
        || (["kalau ", "jika ", "apabila "].iter().any(|p| normalized.starts_with(p))
            && lower.trim_end().ends_with(','))
    {
        return Some(BoundaryRisk::OpenConditional);
    }
    if matches!(last, "dan" | "atau" | "tetapi" | "tapi" | "yang") {
        return Some(BoundaryRisk::OpenConnector);
    }
    None
}

/// An advisory semantic hint must never turn stable ASR text into a source
/// audio boundary. No word timestamps exist in the current Whisper worker.
/// On final whole-window evidence, the producer's finality wins over tail
/// heuristics (a one-word "tidak" may itself be a complete answer).
pub(super) fn assess_boundary(
    candidate: &StableTextCandidate,
    expected_scope: &SourceScope,
    expected_revision: u64,
) -> PolicyDecision {
    if candidate.scope != *expected_scope || candidate.scope.session_id.trim().is_empty() {
        return PolicyDecision::Wait(WaitReason::InvalidAuthority);
    }
    if expected_revision == 0 || candidate.revision != expected_revision {
        return PolicyDecision::Wait(WaitReason::StaleRevision);
    }
    if candidate.text.trim().is_empty()
        || candidate.text.len() > MAX_CANDIDATE_BYTES
        || candidate.text.contains('\0')
    {
        return PolicyDecision::Wait(WaitReason::InvalidText);
    }

    if candidate.evidence == StabilityEvidence::GrowingWindowAgreement {
        if let Some(risk) = boundary_risk(&candidate.text) {
            return PolicyDecision::Wait(WaitReason::OpenMeaning(risk));
        }
        // Even well-punctuated text and a WHOLE observation clock cannot
        // identify the capture samples belonging to a provisional prefix.
        return PolicyDecision::Wait(WaitReason::NoAlignedPartialBoundary);
    }

    let Some(span) = candidate.whole_final_span.as_ref() else {
        return PolicyDecision::Wait(WaitReason::MissingFinalSpan);
    };
    if span.scope != candidate.scope
        || span.asr_window != candidate.observation_window
        || candidate.capture_window_bounds != Some(span.capture_bounds)
        || span.asr_window.end_sample <= span.asr_window.start_sample
        || span.capture_bounds.end_upper_sample <= span.capture_bounds.start_lower_sample
        || !(8_000..=192_000).contains(&span.capture_bounds.sample_rate_hz)
    {
        return PolicyDecision::Wait(WaitReason::InconsistentFinalSpan);
    }

    PolicyDecision::CommitReady(CommitCandidate {
        scope: candidate.scope.clone(),
        revision: candidate.revision,
        source_span: span.clone(),
        text: candidate.text.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::source_observations::{
        AsrObservation, AudioWindow, CaptureClockSegment, ObservationOutcome,
        SourceLane, SourceStabilizer,
    };

    fn scope(epoch: u64) -> SourceScope {
        SourceScope {
            session_id: "meeting-a".into(),
            generation: Some(12),
            lane: SourceLane::You,
            speech_id: 9,
            audio_epoch: epoch,
        }
    }

    fn final_candidate(source: &SourceScope, text: &str) -> StableTextCandidate {
        let mut stabilizer = SourceStabilizer::new(source.clone()).unwrap();
        let window = AudioWindow { start_sample: 0, end_sample: 16_000 };
        let capture = CaptureClockSegment {
            scope: source.clone(),
            asr_window: window,
            capture_window: AudioWindow { start_sample: 48_000, end_sample: 96_000 },
            capture_rate_hz: 48_000,
        };
        let outcome = stabilizer.observe_with_capture_mapping(AsrObservation {
            scope: source.clone(), revision: 3, window,
            transcript: text.into(), finalized: true,
        }, Some(&capture)).unwrap();
        let ObservationOutcome::Stable(candidate) = outcome else {
            panic!("canonical finalized source expected");
        };
        candidate
    }

    fn stable_partial(source: &SourceScope, first: &str, second: &str) -> StableTextCandidate {
        let mut stabilizer = SourceStabilizer::new(source.clone()).unwrap();
        let before = AudioWindow { start_sample: 0, end_sample: 8_000 };
        let after = AudioWindow { start_sample: 0, end_sample: 12_000 };
        assert_eq!(stabilizer.observe(AsrObservation {
            scope: source.clone(), revision: 1, window: before,
            transcript: first.into(), finalized: false,
        }), Ok(ObservationOutcome::Pending));
        let outcome = stabilizer.observe(AsrObservation {
            scope: source.clone(), revision: 2, window: after,
            transcript: second.into(), finalized: false,
        }).unwrap();
        let ObservationOutcome::Stable(candidate) = outcome else {
            panic!("anchored stable text expected");
        };
        candidate
    }

    #[test]
    fn finalized_whole_source_can_be_nominated_but_not_played_by_policy() {
        let id = scope(2);
        let candidate = final_candidate(&id, "Tidak.");
        let decision = assess_boundary(&candidate, &id, 3);
        let PolicyDecision::CommitReady(ready) = decision else {
            panic!("finalized span expected");
        };
        assert_eq!(ready.scope, id);
        assert_eq!(ready.revision, 3);
        assert_eq!(ready.text, "Tidak.");
        assert_eq!(ready.source_span.capture_bounds.start_lower_sample, 48_000);
        // Merely being 'ready' does not insert into canonical committed_turns,
        // translate, synthesize or grant Meeting playback authority.
    }

    #[test]
    fn punctuation_and_stable_prefix_cannot_replace_missing_word_alignment() {
        let id = scope(1);
        let partial = stable_partial(&id, "Server aktif.", "Server aktif. besok");
        assert_eq!(partial.text, "Server aktif.");
        assert_eq!(partial.whole_final_span, None);
        assert_eq!(assess_boundary(&partial, &id, 2),
            PolicyDecision::Wait(WaitReason::NoAlignedPartialBoundary));
    }

    #[test]
    fn open_conditional_negation_and_correction_are_wait_hints_only() {
        let id = scope(4);
        for (first, second, risk) in [
            ("Saya tidak", "Saya tidak setuju", BoundaryRisk::TrailingNegation),
            ("Kalau sudah", "Kalau sudah selesai", BoundaryRisk::OpenConditional),
            ("Maksud saya", "Maksud saya besok", BoundaryRisk::PossibleRepair),
            ("Kita dan", "Kita dan mereka", BoundaryRisk::OpenConnector),
        ] {
            let partial = stable_partial(&id, first, second);
            assert_eq!(assess_boundary(&partial, &id, 2),
                PolicyDecision::Wait(WaitReason::OpenMeaning(risk)));
        }
    }

    #[test]
    fn wrong_epoch_revision_and_missing_or_corrupt_alignment_are_rejected() {
        let id = scope(5);
        let candidate = final_candidate(&id, "Versi 1.21.1 sudah tersedia.");
        assert_eq!(assess_boundary(&candidate, &scope(6), 3),
            PolicyDecision::Wait(WaitReason::InvalidAuthority));
        assert_eq!(assess_boundary(&candidate, &id, 2),
            PolicyDecision::Wait(WaitReason::StaleRevision));
        let mut unaligned = candidate.clone();
        unaligned.whole_final_span = None;
        assert_eq!(assess_boundary(&unaligned, &id, 3),
            PolicyDecision::Wait(WaitReason::MissingFinalSpan));
        let mut tampered = candidate;
        tampered.whole_final_span.as_mut().unwrap().asr_window.end_sample -= 1;
        assert_eq!(assess_boundary(&tampered, &id, 3),
            PolicyDecision::Wait(WaitReason::InconsistentFinalSpan));
    }

    #[test]
    fn correction_and_clause_hints_never_certify_truth_or_force_finalized_rejection() {
        let id = scope(7);
        assert_eq!(boundary_risk("Kalau sudah selesai, kita mulai sekarang."), None);
        assert_eq!(boundary_risk("maksud saya"), Some(BoundaryRisk::PossibleRepair));
        assert_eq!(boundary_risk("Saya bukan"), Some(BoundaryRisk::TrailingNegation));
        assert_eq!(boundary_risk("Pakai versi 1.21.1."), None);
        // A finalized reply can end with a standalone negative word; the
        // weak lexical hint must not become a universal rejection rule.
        assert!(matches!(
            assess_boundary(&final_candidate(&id, "Tidak."), &id, 3),
            PolicyDecision::CommitReady(_),
        ));
    }
}
