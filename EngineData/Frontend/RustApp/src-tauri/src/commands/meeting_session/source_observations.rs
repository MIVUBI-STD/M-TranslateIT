//! Bounded ASR observation/stability candidate for Balanced Interpreter.
//! Only compiled by the Rust test harness until the canonical Meeting owner
//! can supply audio-epoch sample positions and authorize progressive commits.
//! Agreement across growing windows is TEXT evidence, not word/audio alignment.

use crate::engine::audio::finalized_utterance::FinalizedMeetingUtterance;
use crate::engine::audio::evidence::AudioEvidenceReport;
use crate::engine::audio::vad::runtime_vad_profile;

const ASR_RATE_HZ: u32 = 16_000;
const MAX_OBSERVATION_BYTES: usize = 8 * 1024;
const MAX_WINDOW_SAMPLES_16K: u64 = 16_000 * 60;
const MAX_TIMING_PROBE_WORDS: usize = 512;
const MAX_TIMING_PROBE_BYTES: usize = 16 * 1024;
// Strict limits for the test-only, non-producing long-speech *proposal*.
// Word timestamps/punctuation are model estimates and cannot authorize
// translation, an audio cut, or playback without separate quality acceptance.
const PROPOSED_MAX_SEGMENT_SAMPLES: u64 = 16_000 * 20;
const PROPOSED_MIN_SEGMENT_SAMPLES: u64 = 16_000 * 3;
const PROPOSED_MIN_QUIET_SAMPLES: u64 = 16_000 * 160 / 1_000;
const QUIET_PROBE_FRAME_SAMPLES: usize = 16_000 * 20 / 1_000;

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

// Advisory only: no samples are copied or removed and no Meeting owner is
// allowed to commit these ranges. Each exact ASR range maps to an approximate
// source-clock interval; the full set covers the original window once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LongSpeechSegmentProposal {
    pub asr_window: AudioWindow,
    pub capture_bounds: CaptureWindowBounds,
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
        if window == self.asr_window {
            // Only this WHOLE accepted producer range has exact sample
            // endpoints; partial windows remain ratio-based approximations.
            return Ok(CaptureWindowBounds {
                start_lower_sample: self.capture_window.start_sample,
                end_upper_sample: self.capture_window.end_sample,
                sample_rate_hz: self.capture_rate_hz,
            });
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

// Internal bridge: real finalized VAD metadata -> same existing ASR
// observation domain. Transcript text is supplied separately by ASR.
pub(super) fn finalized_capture_clock(
    utterance: &FinalizedMeetingUtterance,
) -> Option<(SourceScope, AudioWindow, CaptureClockSegment)> {
    let p = utterance.capture_provenance?;
    if utterance.frame.sample_rate_hz != ASR_RATE_HZ
        || utterance.frame.channels != 1
        || utterance.frame.samples.len() as u64 != p.asr_sample_count
        || utterance.utterance_id == 0
        || p.audio_epoch == 0
        || p.input_end_sample <= p.input_start_sample
    {
        return None;
    }
    let lane = match utterance.lane.as_str() {
        "you" if utterance.generation.is_some() => SourceLane::You,
        "incoming" if utterance.generation.is_none() => SourceLane::Incoming,
        _ => return None,
    };
    let scope = SourceScope {
        session_id: utterance.session_id.clone(),
        generation: utterance.generation,
        lane,
        speech_id: utterance.utterance_id,
        audio_epoch: p.audio_epoch,
    };
    let asr_window = AudioWindow { start_sample: 0, end_sample: p.asr_sample_count };
    let mapping = CaptureClockSegment {
        scope: scope.clone(),
        asr_window,
        capture_window: AudioWindow {
            start_sample: p.input_start_sample,
            end_sample: p.input_end_sample,
        },
        capture_rate_hz: p.input_rate_hz,
    };
    mapping.map_window(&scope, asr_window).ok()?;
    Some((scope, asr_window, mapping))
}

// Estimated offsets from C.2's opt-in 16-kHz WAV; never a committed source span.
#[derive(Debug, Clone)]
pub(super) struct ModelEstimatedWord {
    pub text: String,
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
    // A nonfinal stable-prefix candidate must retain its authoritative
    // session/generation/speech/epoch identity even without word alignment.
    pub scope: SourceScope,
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
    UnmatchedWordTiming,
    InvalidWordTiming,
    NoSafeSegmentBoundary,
    InvalidQuietEvidence,
    UnavailableFinalizedSource,
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

    /// Pure, test-only review of model-estimated words against one current ASR
    /// observation and its producer-supplied capture clock. No commit/playback.
    pub fn inspect_estimated_words(
        &self,
        scope: &SourceScope,
        revision: u64,
        window: AudioWindow,
        frame_count: u64,
        words: &[ModelEstimatedWord],
        capture_map: &CaptureClockSegment,
    ) -> Result<Vec<CaptureWindowBounds>, ObservationError> {
        let last = self.latest.as_ref().ok_or(ObservationError::UnmatchedWordTiming)?;
        if scope != &self.scope
            || revision != last.revision
            || window != last.window
            || frame_count != window.end_sample - window.start_sample
            || words.is_empty()
            || words.len() > MAX_TIMING_PROBE_WORDS
            || capture_map.scope != self.scope
            || capture_map.asr_window != window
        {
            return Err(ObservationError::UnmatchedWordTiming);
        }
        capture_map.map_window(scope, window)?;
        let mut joined = String::new();
        let mut prev_end = 0;
        let mut mapped = Vec::with_capacity(words.len());
        for word in words {
            if word.text.trim().is_empty()
                || word.text.len() > 256
                || word.text.contains('\0')
                || word.start_sample < prev_end
                || word.end_sample <= word.start_sample
                || word.end_sample > frame_count
                || joined.len().saturating_add(word.text.len()) > MAX_TIMING_PROBE_BYTES
            {
                return Err(ObservationError::InvalidWordTiming);
            }
            joined.push_str(&word.text);
            let mapped_window = AudioWindow {
                start_sample: window.start_sample.checked_add(word.start_sample)
                    .ok_or(ObservationError::InvalidWordTiming)?,
                end_sample: window.start_sample.checked_add(word.end_sample)
                    .ok_or(ObservationError::InvalidWordTiming)?,
            };
            mapped.push(capture_map.map_window(scope, mapped_window)?);
            prev_end = word.end_sample;
        }
        // Same whitespace normalization as Python's transcript extraction.
        if joined.split_whitespace().collect::<Vec<_>>().join(" ")
            != last.transcript.split_whitespace().collect::<Vec<_>>().join(" ")
        {
            return Err(ObservationError::UnmatchedWordTiming);
        }
        Ok(mapped)
    }

    /// Test-only bridge from an actual finalized producer frame to the
    /// previously isolated segmentation proposal. This never asks Whisper
    /// to infer, writes a WAV, mutates the producer, or commits speech.
    ///
    /// The optional worker alignment probe supplies estimated words; those
    /// estimates are still untrusted. Quiet is measured directly from the
    /// *matching* 16-kHz finalized PCM samples, not from ASR word gaps.
    pub fn propose_from_finalized_pcm(
        &self,
        utterance: &FinalizedMeetingUtterance,
        revision: u64,
        words: &[ModelEstimatedWord],
    ) -> Result<Vec<LongSpeechSegmentProposal>, ObservationError> {
        let (scope, window, capture_map) = finalized_capture_clock(utterance)
            .ok_or(ObservationError::UnavailableFinalizedSource)?;
        let pcm = &utterance.frame.samples;
        if window.end_sample - window.start_sample != pcm.len() as u64 {
            return Err(ObservationError::UnavailableFinalizedSource);
        }
        let quiet = measured_pcm_quiet_windows(pcm, window)?;
        self.propose_long_speech_segments(
            &scope, revision, window, pcm.len() as u64, words, &capture_map, &quiet,
        )
    }

    /// Experimental bounded segmentation proposal. Never transcribes, commits,
    /// translates or plays. We require matching current-word evidence plus a
    /// independently producer-observed quiet interval after a sentence-ending
    /// word; the ASR model's timestamps alone are NEVER evidence of silence.
    /// Even the paired signals are NOT semantic truth or output authorization.
    /// A 60 s window with no safe candidate is rejected, never arbitrarily cut.
    pub fn propose_long_speech_segments(
        &self,
        scope: &SourceScope,
        revision: u64,
        window: AudioWindow,
        frame_count: u64,
        words: &[ModelEstimatedWord],
        capture_map: &CaptureClockSegment,
        pcm_quiet_windows: &[AudioWindow],
    ) -> Result<Vec<LongSpeechSegmentProposal>, ObservationError> {
        // This already rejects foreign sessions/epochs, stale revisions,
        // mismatched source text, gaps in the capture clock, and invalid word
        // positions. Word estimates remain untrusted even after validation.
        self.inspect_estimated_words(
            scope, revision, window, frame_count, words, capture_map,
        )?;
        // These ranges must originate from a future authoritative PCM/VAD
        // capture owner. This test-only function cannot verify their origin.
        // Neither blank input nor untrusted ASR word gaps can substitute.
        if pcm_quiet_windows.len() > MAX_TIMING_PROBE_WORDS
            || pcm_quiet_windows.iter().any(|quiet| {
                quiet.start_sample < window.start_sample
                    || quiet.end_sample > window.end_sample
                    || quiet.end_sample <= quiet.start_sample
            })
            || pcm_quiet_windows.windows(2).any(|pair| {
                pair[0].end_sample > pair[1].start_sample
            })
        {
            return Err(ObservationError::InvalidQuietEvidence);
        }
        let mut start = window.start_sample;
        let mut segments = Vec::new();
        while window.end_sample - start > PROPOSED_MAX_SEGMENT_SAMPLES {
            let limit = start + PROPOSED_MAX_SEGMENT_SAMPLES;
            let mut latest_candidate = None;
            for pair in words.windows(2) {
                let previous = &pair[0];
                let next = &pair[1];
                let proposed = window.start_sample
                    .checked_add(next.start_sample)
                    .ok_or(ObservationError::InvalidWordTiming)?;
                if proposed <= start + PROPOSED_MIN_SEGMENT_SAMPLES
                    || proposed > limit
                    || window.end_sample - proposed < PROPOSED_MIN_SEGMENT_SAMPLES
                    || next.start_sample.saturating_sub(previous.end_sample)
                        < PROPOSED_MIN_QUIET_SAMPLES
                {
                    continue;
                }
                let previous_end = window.start_sample
                    .checked_add(previous.end_sample)
                    .ok_or(ObservationError::InvalidWordTiming)?;
                let next_start = proposed;
                if !pcm_quiet_windows.iter().any(|quiet| {
                    quiet.start_sample >= previous_end
                        && quiet.end_sample <= next_start
                        && quiet.end_sample - quiet.start_sample
                            >= PROPOSED_MIN_QUIET_SAMPLES
                }) {
                    continue;
                }
                // Punctuation only proposes a boundary; even a period plus
                // estimated silence cannot establish semantic independence.
                if !previous.text.trim_end().chars().last()
                    .is_some_and(|ch| matches!(ch, '.' | '?' | '!' | ';')) {
                    continue;
                }
                latest_candidate = Some(proposed);
            }
            let end = latest_candidate.ok_or(ObservationError::NoSafeSegmentBoundary)?;
            let asr_window = AudioWindow { start_sample: start, end_sample: end };
            segments.push(LongSpeechSegmentProposal {
                asr_window,
                capture_bounds: capture_map.map_window(scope, asr_window)?,
            });
            start = end;
        }
        let final_window = AudioWindow {
            start_sample: start,
            end_sample: window.end_sample,
        };
        if final_window.end_sample - final_window.start_sample < PROPOSED_MIN_SEGMENT_SAMPLES {
            return Err(ObservationError::NoSafeSegmentBoundary);
        }
        segments.push(LongSpeechSegmentProposal {
            asr_window: final_window,
            capture_bounds: capture_map.map_window(scope, final_window)?,
        });
        Ok(segments)
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
                scope: self.scope.clone(),
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
                scope: self.scope.clone(),
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


 // PCM-derived quiet is a *proposal* signal, not speech-recognition or a
 // semantic boundary. A 20-ms frame must be plainly below BOTH canonical
 // VAD amplitude thresholds. Clipped, non-finite or out-of-range PCM fails
 // closed rather than being sanitized into evidence of silence.
fn measured_pcm_quiet_windows(
    pcm: &[f32],
    window: AudioWindow,
) -> Result<Vec<AudioWindow>, ObservationError> {
    if pcm.is_empty()
        || pcm.len() as u64 != window.end_sample.saturating_sub(window.start_sample)
        || pcm.len() as u64 > MAX_WINDOW_SAMPLES_16K
        || pcm.iter().any(|sample| !sample.is_finite() || sample.abs() > 1.0)
    {
        return Err(ObservationError::InvalidQuietEvidence);
    }
    let gate = runtime_vad_profile().gate;
    let mut quiet = Vec::new();
    let mut start = None;
    let mut accepted_end = window.start_sample;
    for (index, frame) in pcm.chunks_exact(QUIET_PROBE_FRAME_SAMPLES).enumerate() {
        let frame_start = window.start_sample
            + (index * QUIET_PROBE_FRAME_SAMPLES) as u64;
        let frame_end = frame_start + QUIET_PROBE_FRAME_SAMPLES as u64;
        let evidence = AudioEvidenceReport::from_samples(frame);
        let is_quiet = evidence.rms < gate.min_rms * 0.5
            && evidence.peak < gate.min_peak * 0.5
            && evidence.clipping_ratio == 0.0;
        if is_quiet {
            if start.is_none() { start = Some(frame_start); }
            accepted_end = frame_end;
        } else if let Some(begin) = start.take() {
            if accepted_end - begin >= PROPOSED_MIN_QUIET_SAMPLES {
                quiet.push(AudioWindow { start_sample: begin, end_sample: accepted_end });
                if quiet.len() > MAX_TIMING_PROBE_WORDS {
                    return Err(ObservationError::InvalidQuietEvidence);
                }
            }
        }
    }
    if let Some(begin) = start {
        if accepted_end - begin >= PROPOSED_MIN_QUIET_SAMPLES {
            quiet.push(AudioWindow { start_sample: begin, end_sample: accepted_end });
            if quiet.len() > MAX_TIMING_PROBE_WORDS {
                return Err(ObservationError::InvalidQuietEvidence);
            }
        }
    }
    Ok(quiet)
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
                scope: scope.clone(),
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
                scope: scope.clone(),
                text: "Mulai".into(),
                revision: 2,
                evidence: StabilityEvidence::GrowingWindowAgreement,
                observation_window: AudioWindow { start_sample: 0, end_sample: 12_000 },
                capture_window_bounds: None,
                whole_final_span: None,
            }),
        );
    }

    #[test]
    fn canonical_finalized_metadata_maps_whole_asr_frame_not_words() {
        use crate::engine::audio::finalized_utterance::{
            FinalizedCaptureProvenance, FinalizedMeetingUtterance,
        };
        use crate::engine::audio::AudioFrame;
        use std::time::Instant;
        let now = Instant::now();
        let utterance = FinalizedMeetingUtterance {
            session_id: "s1".into(), sequence: 1, lane: "you".into(),
            generation: Some(7), utterance_id: 31, finalized_at: now,
            enqueued_at: now, finalized_unix_ms: 0, speech_boundary_ms: 0,
            speech_duration_ms: 1000, finalization_ms: 0,
            capture_provenance: Some(FinalizedCaptureProvenance {
                audio_epoch: 3, input_rate_hz: 44_100,
                input_start_sample: 2_000, input_end_sample: 46_100,
                asr_sample_count: 16_000,
            }),
            frame: AudioFrame {
                sample_rate_hz: 16_000, channels: 1, samples: vec![0.0; 16_000],
            },
        };
        let (scope, window, clock) =
            finalized_capture_clock(&utterance).expect("valid native clock");
        assert_eq!(clock.map_window(&scope, window).unwrap(), CaptureWindowBounds {
            start_lower_sample: 2_000,
            end_upper_sample: 46_100,
            sample_rate_hz: 44_100,
        });
        let mut stabilizer = SourceStabilizer::new(scope.clone()).unwrap();
        let result = stabilizer.observe_with_capture_mapping(
            observation(&scope, 1, 0, 16_000, "Kita mulai besok.", true),
            Some(&clock),
        ).unwrap();
        let ObservationOutcome::Stable(candidate) = result else { panic!("final text") };
        assert_eq!(candidate.whole_final_span.unwrap().capture_bounds.end_upper_sample, 46_100);
        let mut bad = utterance;
        bad.capture_provenance.as_mut().unwrap().asr_sample_count = 8_000;
        assert!(finalized_capture_clock(&bad).is_none());
    }

    #[test]
    fn estimated_word_ranges_map_only_against_current_source_clock() {
        let id = scope(21);
        let mut tracker = SourceStabilizer::new(id.clone()).unwrap();
        let window = AudioWindow { start_sample: 4_000, end_sample: 12_000 };
        let clock = map(&id, 4_000, 12_000, 12_000, 36_000, 48_000);
        let words = vec![
            ModelEstimatedWord { text: " Server".into(), start_sample: 1_000, end_sample: 2_000 },
            ModelEstimatedWord { text: " aktif".into(), start_sample: 2_100, end_sample: 4_000 },
        ];
        assert_eq!(tracker.inspect_estimated_words(&id, 1, window, 8_000, &words, &clock),
            Err(ObservationError::UnmatchedWordTiming));
        tracker.observe(observation(&id, 1, 4_000, 12_000, "Server aktif", false)).unwrap();
        let mapped = tracker.inspect_estimated_words(&id, 1, window, 8_000, &words, &clock).unwrap();
        assert_eq!(mapped[0], CaptureWindowBounds {
            start_lower_sample: 15_000, end_upper_sample: 18_000, sample_rate_hz: 48_000,
        });
        assert_eq!(mapped.len(), 2);
        assert_eq!(tracker.inspect_estimated_words(&id, 2, window, 8_000, &words, &clock),
            Err(ObservationError::UnmatchedWordTiming));
        let other_epoch = SourceScope { audio_epoch: 22, ..id.clone() };
        assert_eq!(tracker.inspect_estimated_words(&other_epoch, 1, window, 8_000, &words, &clock),
            Err(ObservationError::UnmatchedWordTiming));
    }

    #[test]
    fn word_overlap_bad_text_frame_count_and_foreign_clock_fail_closed() {
        let id = scope(23);
        let mut tracker = SourceStabilizer::new(id.clone()).unwrap();
        let window = AudioWindow { start_sample: 0, end_sample: 8_000 };
        let clock = map(&id, 0, 8_000, 0, 24_000, 48_000);
        tracker.observe(observation(&id, 1, 0, 8_000, "Server aktif", false)).unwrap();
        let mut words = vec![
            ModelEstimatedWord { text: " Server".into(), start_sample: 1_000, end_sample: 2_000 },
            ModelEstimatedWord { text: " aktif".into(), start_sample: 2_100, end_sample: 3_000 },
        ];
        words[1].start_sample = 1_999;
        assert_eq!(tracker.inspect_estimated_words(&id, 1, window, 8_000, &words, &clock),
            Err(ObservationError::InvalidWordTiming));
        words[1].start_sample = 2_100;
        words[1].text = " tidak".into();
        assert_eq!(tracker.inspect_estimated_words(&id, 1, window, 8_000, &words, &clock),
            Err(ObservationError::UnmatchedWordTiming));
        assert_eq!(tracker.inspect_estimated_words(&id, 1, window, 4_000, &words, &clock),
            Err(ObservationError::UnmatchedWordTiming));
        words[1].text = " aktif".into();
        let wrong_clock = map(&SourceScope { audio_epoch: 24, ..id.clone() },
            0, 8_000, 0, 24_000, 48_000);
        assert_eq!(tracker.inspect_estimated_words(&id, 1, window, 8_000, &words, &wrong_clock),
            Err(ObservationError::UnmatchedWordTiming));
        tracker.observe(observation(&id, 2, 0, 9_000, "Server aktif lagi", false)).unwrap();
        assert_eq!(tracker.inspect_estimated_words(&id, 1, window, 8_000, &words, &clock),
            Err(ObservationError::UnmatchedWordTiming));
    }


    // This is a source-only proposal fixture, not a production interpreter
    // feature: it cannot authorize any ASR/TTS request or spoken playback.
    #[test]
    fn long_speech_proposal_preserves_one_nonoverlapping_audio_window() {
        let id = scope(51);
        let mut tracker = SourceStabilizer::new(id.clone()).unwrap();
        let duration_samples = 51 * u64::from(ASR_RATE_HZ);
        let window = AudioWindow { start_sample: 0, end_sample: duration_samples };
        let clock = map(&id, 0, duration_samples, 0, duration_samples * 3, 48_000);
        let w = |text: &str, begin_ms: u64, end_ms: u64| ModelEstimatedWord {
            text: text.into(),
            start_sample: begin_ms * u64::from(ASR_RATE_HZ) / 1_000,
            end_sample: end_ms * u64::from(ASR_RATE_HZ) / 1_000,
        };
        let quiet = |begin_ms: u64, end_ms: u64| AudioWindow {
            start_sample: begin_ms * u64::from(ASR_RATE_HZ) / 1_000,
            end_sample: end_ms * u64::from(ASR_RATE_HZ) / 1_000,
        };
        // These exact ASR-clock quiet observations would have to come from
        // the producer, not from Whisper word timestamps in a live implementation.
        let quiet_windows = [
            quiet(14_600, 14_900),
            quiet(29_600, 29_900),
            quiet(44_600, 44_900),
        ];
        let words = vec![
            w("Start", 0, 1_000),
            w(" now.", 1_000, 14_500),
            w(" Again", 15_000, 16_000),
            w(" go.", 16_000, 29_500),
            w(" Later", 30_000, 31_000),
            w(" done.", 31_000, 44_500),
            w(" Finish", 45_000, 46_000),
            w(" okay.", 46_000, 50_000),
        ];
        tracker.observe(observation(&id, 1, 0, duration_samples,
            "Start now. Again go. Later done. Finish okay.", false)).unwrap();
        // Whisper's estimated word gap alone never passes the candidate gate.
        assert_eq!(tracker.propose_long_speech_segments(
            &id, 1, window, duration_samples, &words, &clock, &[],
        ), Err(ObservationError::NoSafeSegmentBoundary));
        let unsupported_quiet = [quiet(13_000, 13_400)];
        assert_eq!(tracker.propose_long_speech_segments(
            &id, 1, window, duration_samples, &words, &clock, &unsupported_quiet,
        ), Err(ObservationError::NoSafeSegmentBoundary));
        let proposals = tracker.propose_long_speech_segments(
            &id, 1, window, duration_samples, &words, &clock, &quiet_windows,
        ).expect("aligned words plus independent PCM quiet yield proposals");
        let secs: Vec<(u64, u64)> = proposals.iter()
            .map(|entry| (entry.asr_window.start_sample / u64::from(ASR_RATE_HZ),
                entry.asr_window.end_sample / u64::from(ASR_RATE_HZ)))
            .collect();
        assert_eq!(secs, vec![(0, 15), (15, 30), (30, 45), (45, 51)]);
        assert_eq!(proposals.first().unwrap().asr_window.start_sample, window.start_sample);
        assert_eq!(proposals.last().unwrap().asr_window.end_sample, window.end_sample);
        for pair in proposals.windows(2) {
            assert_eq!(pair[0].asr_window.end_sample, pair[1].asr_window.start_sample);
            assert_eq!(pair[0].capture_bounds.end_upper_sample,
                pair[1].capture_bounds.start_lower_sample);
        }
        assert!(proposals.iter().all(|entry|
            entry.asr_window.end_sample - entry.asr_window.start_sample
                <= PROPOSED_MAX_SEGMENT_SAMPLES));
    }

    #[test]
    fn finalized_pcm_bridge_requires_actual_quiet_not_just_whisper_word_gaps() {
        use crate::engine::audio::finalized_utterance::{
            FinalizedCaptureProvenance, FinalizedMeetingUtterance,
        };
        use crate::engine::audio::AudioFrame;
        use std::time::Instant;

        let id = scope(71);
        let rate = ASR_RATE_HZ as usize;
        let mut samples = vec![0.12_f32; rate * 51];
        for (from_ms, until_ms) in [
            (14_600_usize, 14_900_usize),
            (29_600, 29_900),
            (44_600, 44_900),
        ] {
            samples[from_ms * rate / 1_000..until_ms * rate / 1_000].fill(0.0);
        }
        let now = Instant::now();
        let utterance = FinalizedMeetingUtterance {
            session_id: id.session_id.clone(),
            sequence: 1,
            lane: "you".into(),
            generation: id.generation,
            utterance_id: id.speech_id,
            finalized_at: now,
            enqueued_at: now,
            finalized_unix_ms: 0,
            speech_boundary_ms: 0,
            speech_duration_ms: 51_000,
            finalization_ms: 0,
            capture_provenance: Some(FinalizedCaptureProvenance {
                audio_epoch: id.audio_epoch,
                input_rate_hz: 48_000,
                input_start_sample: 12_000,
                input_end_sample: 12_000 + 51 * 48_000,
                asr_sample_count: samples.len() as u64,
            }),
            frame: AudioFrame {
                sample_rate_hz: ASR_RATE_HZ,
                channels: 1,
                samples,
            },
        };
        let (source, window, capture) = finalized_capture_clock(&utterance).unwrap();
        assert_eq!(source, id);
        let mut tracker = SourceStabilizer::new(id.clone()).unwrap();
        tracker.observe_with_capture_mapping(
            observation(&id, 1, 0, window.end_sample,
                "Start now. Again go. Later done. Finish okay.", true),
            Some(&capture),
        ).unwrap();
        let word = |text: &str, from_ms: u64, until_ms: u64| ModelEstimatedWord {
            text: text.into(),
            start_sample: from_ms * u64::from(ASR_RATE_HZ) / 1_000,
            end_sample: until_ms * u64::from(ASR_RATE_HZ) / 1_000,
        };
        // Matches the opt-in worker's estimated word ranges, but does not
        // infer PCM quiet from those estimates.
        let words = [
            word("Start", 0, 1_000),
            word(" now.", 1_000, 14_500),
            word(" Again", 15_000, 16_000),
            word(" go.", 16_000, 29_500),
            word(" Later", 30_000, 31_000),
            word(" done.", 31_000, 44_500),
            word(" Finish", 45_000, 46_000),
            word(" okay.", 46_000, 50_000),
        ];
        let proposal = tracker.propose_from_finalized_pcm(&utterance, 1, &words)
            .expect("matching finalized PCM quiet + estimated words yields proposed cuts");
        let spans: Vec<(u64, u64)> = proposal.iter()
            .map(|part| (part.asr_window.start_sample / u64::from(ASR_RATE_HZ),
                part.asr_window.end_sample / u64::from(ASR_RATE_HZ)))
            .collect();
        assert_eq!(spans, vec![(0, 15), (15, 30), (30, 45), (45, 51)]);
        assert_eq!(proposal[0].capture_bounds.start_lower_sample, 12_000);
        assert_eq!(proposal[3].capture_bounds.end_upper_sample, 12_000 + 51 * 48_000);
        assert!(proposal.windows(2).all(|pair|
            pair[0].capture_bounds.end_upper_sample
                == pair[1].capture_bounds.start_lower_sample));

        let mut loud = utterance.clone();
        loud.frame.samples.fill(0.12);
        assert_eq!(tracker.propose_from_finalized_pcm(&loud, 1, &words),
            Err(ObservationError::NoSafeSegmentBoundary));

        // A single 20-ms burst splits 300 ms of quiet into two pieces too
        // short for a valid natural-boundary proposal.
        let mut interrupted = utterance.clone();
        let at = 14_740 * rate / 1_000;
        interrupted.frame.samples[at..at + QUIET_PROBE_FRAME_SAMPLES].fill(0.12);
        assert_eq!(tracker.propose_from_finalized_pcm(&interrupted, 1, &words),
            Err(ObservationError::NoSafeSegmentBoundary));

        let mut corrupted = utterance.clone();
        corrupted.frame.samples[0] = f32::NAN;
        assert_eq!(tracker.propose_from_finalized_pcm(&corrupted, 1, &words),
            Err(ObservationError::InvalidQuietEvidence));

        let mut wrong_provenance = utterance.clone();
        wrong_provenance.capture_provenance.as_mut().unwrap().asr_sample_count -= 1;
        assert_eq!(tracker.propose_from_finalized_pcm(&wrong_provenance, 1, &words),
            Err(ObservationError::UnavailableFinalizedSource));

        assert_eq!(tracker.propose_from_finalized_pcm(&utterance, 2, &words),
            Err(ObservationError::UnmatchedWordTiming));
        let other = SourceStabilizer::new(scope(72)).unwrap();
        assert_eq!(other.propose_from_finalized_pcm(&utterance, 1, &words),
            Err(ObservationError::UnmatchedWordTiming));
    }

    #[test]
    fn measured_pcm_quiet_windows_rejects_corrupt_or_unbounded_frames() {
        let window = AudioWindow { start_sample: 20, end_sample: 20 + 16_000 };
        assert!(measured_pcm_quiet_windows(&vec![0.0; 16_000], window)
            .is_ok_and(|quiet| quiet == vec![window]));
        assert_eq!(measured_pcm_quiet_windows(&vec![0.0; 15_999], window),
            Err(ObservationError::InvalidQuietEvidence));
        let mut clipped = vec![0.0_f32; 16_000];
        clipped[0] = 1.1;
        assert_eq!(measured_pcm_quiet_windows(&clipped, window),
            Err(ObservationError::InvalidQuietEvidence));
        // 150 ms is below the conservative 160-ms minimum.
        let short = vec![0.0_f32; 16_000 * 150 / 1_000];
        let short_window = AudioWindow {
            start_sample: 0, end_sample: short.len() as u64,
        };
        assert_eq!(measured_pcm_quiet_windows(&short, short_window), Ok(vec![]));
    }

    #[test]
    fn long_speech_proposal_refuses_unknown_words_semantic_gaps_and_foreign_owners() {
        let id = scope(52);
        let mut tracker = SourceStabilizer::new(id.clone()).unwrap();
        let duration_samples = 45 * u64::from(ASR_RATE_HZ);
        let window = AudioWindow { start_sample: 0, end_sample: duration_samples };
        let clock = map(&id, 0, duration_samples, 0, duration_samples * 3, 48_000);
        tracker.observe(observation(
            &id, 1, 0, duration_samples, "Saya tidak setuju", false,
        )).unwrap();
        let quiet_windows = [
            AudioWindow { start_sample: 170_000, end_sample: 190_000 },
            AudioWindow { start_sample: 325_000, end_sample: 340_000 },
        ];
        let words = [
            ModelEstimatedWord { text: "Saya".into(), start_sample: 0, end_sample: 160_000 },
            ModelEstimatedWord { text: " tidak".into(), start_sample: 240_000, end_sample: 320_000 },
            ModelEstimatedWord { text: " setuju".into(), start_sample: 352_000, end_sample: 704_000 },
        ];
        assert_eq!(tracker.propose_long_speech_segments(
            &id, 1, window, duration_samples, &words, &clock, &quiet_windows,
        ), Err(ObservationError::NoSafeSegmentBoundary));
        let foreign = SourceScope { audio_epoch: 53, ..id.clone() };
        assert_eq!(tracker.propose_long_speech_segments(
            &foreign, 1, window, duration_samples, &words, &clock, &quiet_windows,
        ), Err(ObservationError::UnmatchedWordTiming));
        assert_eq!(tracker.propose_long_speech_segments(
            &id, 2, window, duration_samples, &words, &clock, &quiet_windows,
        ), Err(ObservationError::UnmatchedWordTiming));
        let wrong_clock = map(&foreign, 0, duration_samples, 0, duration_samples * 3, 48_000);
        assert_eq!(tracker.propose_long_speech_segments(
            &id, 1, window, duration_samples, &words, &wrong_clock, &quiet_windows,
        ), Err(ObservationError::UnmatchedWordTiming));
        // Estimated silence without an independent PCM observation is
        // explicitly insufficient, even when the word timestamps agree.
        let mut punctuated = words.to_vec();
        punctuated[1].text = " tidak.".into();
        let mut altered = SourceStabilizer::new(id.clone()).unwrap();
        altered.observe(observation(
            &id, 1, 0, duration_samples, "Saya tidak. setuju", false,
        )).unwrap();
        assert_eq!(altered.propose_long_speech_segments(
            &id, 1, window, duration_samples, &punctuated, &clock, &[],
        ), Err(ObservationError::NoSafeSegmentBoundary));
        let invalid_quiet = [AudioWindow {
            start_sample: duration_samples + 1,
            end_sample: duration_samples + 500,
        }];
        assert_eq!(altered.propose_long_speech_segments(
            &id, 1, window, duration_samples, &punctuated, &clock, &invalid_quiet,
        ), Err(ObservationError::InvalidQuietEvidence));
        // Estimated punctuation alone cannot create a valid gap in actual
        // source-time evidence or become a production ASR boundary.
        let mut guessed = words.to_vec();
        guessed[1].text = " tidak.".into();
        assert_eq!(tracker.propose_long_speech_segments(
            &id, 1, window, duration_samples, &guessed, &clock, &quiet_windows,
        ), Err(ObservationError::UnmatchedWordTiming));
    }


}
