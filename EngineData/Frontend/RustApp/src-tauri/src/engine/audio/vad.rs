use serde::{Deserialize, Serialize};

use super::evidence::AudioEvidenceReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadGateConfig {
    pub min_rms: f32,
    pub min_peak: f32,
    pub min_active_frame_ratio: f32,
    pub max_clipping_ratio: f32,
    pub min_speech_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeVadProfile {
    pub name: String,
    pub pre_roll_audio_ms: u32,
    pub minimum_speech_duration_ms: u32,
    pub minimum_silence_duration_ms: u32,
    pub target_chunk_min_ms: u32,
    pub target_chunk_max_ms: u32,
    pub maximum_segment_duration_ms: u32,
    pub partial_asr_enabled: bool,
    pub gate: VadGateConfig,
}

pub fn runtime_vad_profile() -> RuntimeVadProfile {
    RuntimeVadProfile {
        name: "Meeting".to_string(),
        pre_roll_audio_ms: 140,
        minimum_speech_duration_ms: 140,
        minimum_silence_duration_ms: 100,
        target_chunk_min_ms: 320,
        target_chunk_max_ms: 700,
        maximum_segment_duration_ms: 1_500,
        // Legacy profile metadata, not a production ASR cutover. Meeting still
        // processes finalized utterances; this flag alone starts no draft work.
        partial_asr_enabled: true,
        gate: VadGateConfig {
            min_rms: 0.006,
            min_peak: 0.021,
            min_active_frame_ratio: 0.050,
            max_clipping_ratio: 0.025,
            min_speech_ms: 120,
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadGateResult {
    pub accepted: bool,
    pub reason: String,
    pub evidence: AudioEvidenceReport,
}

pub fn evaluate_vad_gate(evidence: AudioEvidenceReport, config: &VadGateConfig) -> VadGateResult {
    if safe_ratio(evidence.clipping_ratio) > safe_ratio(config.max_clipping_ratio) {
        return reject("rejected_clipping", evidence);
    }
    if safe_ratio(evidence.rms) < safe_ratio(config.min_rms) {
        return reject("rejected_low_energy", evidence);
    }
    if safe_ratio(evidence.peak) < safe_ratio(config.min_peak) {
        return reject("rejected_low_peak", evidence);
    }
    if safe_ratio(evidence.active_frame_ratio) < safe_ratio(config.min_active_frame_ratio) {
        return reject("rejected_unconfirmed_speech", evidence);
    }

    VadGateResult {
        accepted: true,
        reason: "accepted_audio_evidence".to_string(),
        evidence,
    }
}

fn safe_metric(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

fn safe_ratio(value: f32) -> f32 {
    safe_metric(value).clamp(0.0, 1.0)
}

fn reject(reason: &str, evidence: AudioEvidenceReport) -> VadGateResult {
    VadGateResult {
        accepted: false,
        reason: reason.to_string(),
        evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::NAN;

    fn fixture_evidence(rms: f32, peak: f32, active_ratio: f32, clipping_ratio: f32) -> AudioEvidenceReport {
        AudioEvidenceReport {
            reason: "unit_fixture".to_string(),
            rms,
            peak,
            mean_abs: rms * 0.8,
            peak_to_rms_ratio: if rms > 0.0 { peak / rms } else { 0.0 },
            speech_to_noise_gap: 1.0,
            voiced_frame_ratio: active_ratio,
            zero_crossing_rate: 0.1,
            frame_energy_concentration: 0.5,
            frame_active_ratio: active_ratio,
            active_frame_ratio: active_ratio,
            impulse_edge_ratio: 0.0,
            clipping_ratio,
        }
    }

    #[test]
    fn meeting_profile_is_the_single_threshold_source() {
        let profile = runtime_vad_profile();
        assert_eq!(profile.name, "Meeting");
        assert_eq!(profile.gate.min_rms, 0.006);
        assert_eq!(profile.gate.min_peak, 0.021);
        assert_eq!(profile.gate.min_active_frame_ratio, 0.050);
        assert_eq!(profile.gate.max_clipping_ratio, 0.025);
        assert_eq!(profile.gate.min_speech_ms, 120);
        assert_eq!(profile.maximum_segment_duration_ms, 1_500);
    }

    #[test]
    fn gate_accepts_clear_speech_evidence() {
        let result = evaluate_vad_gate(fixture_evidence(0.12, 0.40, 0.90, 0.0), &runtime_vad_profile().gate);
        assert!(result.accepted, "{}", result.reason);
        assert_eq!(result.reason, "accepted_audio_evidence");
    }

    #[test]
    fn gate_rejects_clipping_first_even_when_loud() {
        let result = evaluate_vad_gate(fixture_evidence(0.90, 1.00, 1.00, 0.90), &runtime_vad_profile().gate);
        assert!(!result.accepted);
        assert_eq!(result.reason, "rejected_clipping");
    }

    #[test]
    fn gate_rejects_each_missing_dimension_with_named_reason() {
        let profile = runtime_vad_profile().gate;
        let cases = [
            (fixture_evidence(0.0005, 0.40, 0.90, 0.0), "rejected_low_energy"),
            (fixture_evidence(0.12, 0.010, 0.90, 0.0), "rejected_low_peak"),
            (fixture_evidence(0.12, 0.40, 0.010, 0.0), "rejected_unconfirmed_speech"),
        ];
        for (evidence, expected_reason) in cases {
            let result = evaluate_vad_gate(evidence, &profile);
            assert!(!result.accepted);
            assert_eq!(result.reason, expected_reason);
        }
    }

    #[test]
    fn non_finite_metrics_fail_closed_instead_of_panicking() {
        let result = evaluate_vad_gate(fixture_evidence(NAN, NAN, NAN, NAN), &runtime_vad_profile().gate);
        assert!(!result.accepted);
        assert_eq!(result.reason, "rejected_low_energy");
    }
}
