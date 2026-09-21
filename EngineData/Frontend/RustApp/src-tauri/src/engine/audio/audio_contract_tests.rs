use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use super::finalized_utterance::{
    clear_finalized_incoming_utterance_producer, clear_finalized_meeting_sequence,
    clear_finalized_outbound_utterance_producer, evicted_pending_utterance_count,
    observe_finalized_incoming_f32_samples, observe_finalized_outbound_f32_samples,
    reset_finalized_incoming_utterance_producer, reset_finalized_meeting_sequence,
    reset_finalized_outbound_utterance_producer, try_take_finalized_incoming_utterance,
    wait_take_finalized_outbound_utterance, FinalizedMeetingUtterance,
};
use super::live_audio_buffer::{
    append_live_f32_samples, clear_live_audio_buffer, live_audio_buffer_status,
    reset_live_audio_buffer,
};
use super::live_segment_writer::{
    write_finalized_incoming_utterance_wav, write_finalized_outbound_utterance_wav,
};
use super::{duration_ms, AudioFrame, TARGET_CHANNELS, TARGET_SAMPLE_RATE_HZ};
use crate::engine::runtime_state::{
    begin_application_meeting_session, clear_runtime_session_state,
    revoke_runtime_session_authority,
};

static AUDIO_CONTRACT_TEST_SERIAL: OnceLock<Mutex<()>> = OnceLock::new();

fn audio_test_guard() -> MutexGuard<'static, ()> {
    AUDIO_CONTRACT_TEST_SERIAL
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn clear_audio_contract_state() {
    let _ = clear_live_audio_buffer();
    clear_finalized_outbound_utterance_producer();
    clear_finalized_incoming_utterance_producer();
    clear_finalized_meeting_sequence();
    let _ = clear_runtime_session_state();
}

fn speech_of(rate: u32, ms: u32) -> Vec<f32> {
    vec![0.10_f32; rate as usize * ms as usize / 1_000]
}

fn silence_of(rate: u32, ms: u32) -> Vec<f32> {
    vec![0.0_f32; rate as usize * ms as usize / 1_000]
}

fn finalize_incoming_fixture() {
    let speech = speech_of(TARGET_SAMPLE_RATE_HZ, 400);
    let silence = silence_of(TARGET_SAMPLE_RATE_HZ, 400);
    observe_finalized_incoming_f32_samples(&speech, TARGET_SAMPLE_RATE_HZ, 1);
    observe_finalized_incoming_f32_samples(&silence, TARGET_SAMPLE_RATE_HZ, 1);
}

fn finalize_outbound_fixture() {
    let speech = speech_of(TARGET_SAMPLE_RATE_HZ, 400);
    let silence = silence_of(TARGET_SAMPLE_RATE_HZ, 400);
    observe_finalized_outbound_f32_samples(&speech, TARGET_SAMPLE_RATE_HZ, 1);
    observe_finalized_outbound_f32_samples(&silence, TARGET_SAMPLE_RATE_HZ, 1);
}

fn begin_meeting_generation() -> u64 {
    let report = begin_application_meeting_session();
    assert!(report.blocker.is_empty(), "{}", report.blocker);
    report
        .snapshot
        .expect("Meeting authority claim should expose a runtime snapshot")
        .generation
}

fn utterance_fixture(
    lane: &str,
    generation: Option<u64>,
    sample_rate_hz: u32,
    channels: u16,
    sample_count: usize,
) -> FinalizedMeetingUtterance {
    let now = Instant::now();
    FinalizedMeetingUtterance {
        session_id: "audio-contract".to_string(),
        sequence: 1,
        lane: lane.to_string(),
        generation,
        utterance_id: 1,
        finalized_at: now,
        enqueued_at: now,
        finalized_unix_ms: 1,
        speech_boundary_ms: 0,
        speech_duration_ms: u64::from(duration_ms(
            sample_count / usize::from(channels.max(1)),
            sample_rate_hz,
        )),
        finalization_ms: 0,
        frame: AudioFrame {
            sample_rate_hz,
            channels,
            samples: vec![0.0; sample_count],
        },
    }
}

#[test]
fn rolling_buffer_is_bounded_and_format_changes_reset_old_audio() {
    let _serial = audio_test_guard();
    clear_audio_contract_state();

    assert_eq!(duration_ms(0, 0), 0);
    assert_eq!(duration_ms(16_000, 16_000), 1_000);
    assert_eq!(duration_ms(15_999, 16_000), 999);

    let reset = reset_live_audio_buffer(16_000, 2);
    assert_eq!(reset.max_buffer_samples, 32_000);
    assert_eq!(reset.buffered_samples, 0);

    let stereo_samples = vec![0.05_f32; 80_000];
    append_live_f32_samples(&stereo_samples, 16_000, 2);
    let bounded = live_audio_buffer_status();
    assert_eq!(bounded.sample_rate_hz, Some(16_000));
    assert_eq!(bounded.source_channels, Some(2));
    assert_eq!(bounded.max_buffer_samples, 32_000);
    assert_eq!(bounded.buffered_samples, 32_000);
    assert_eq!(bounded.buffered_duration_ms, 2_000);
    assert_eq!(bounded.frames_received, 40_000);
    assert!(bounded.source_downmixed_to_mono);
    assert!(!bounded.requires_resample_to_target);

    append_live_f32_samples(&vec![0.05_f32; 800], 8_000, 1);
    let reset_format = live_audio_buffer_status();
    assert_eq!(reset_format.sample_rate_hz, Some(8_000));
    assert_eq!(reset_format.source_channels, Some(1));
    assert_eq!(reset_format.max_buffer_samples, 16_000);
    assert_eq!(reset_format.buffered_samples, 800);
    assert_eq!(reset_format.buffered_duration_ms, 100);
    assert_eq!(reset_format.frames_received, 800);
    assert!(reset_format.requires_resample_to_target);
    assert!(!reset_format.source_downmixed_to_mono);

    clear_audio_contract_state();
}

#[test]
fn finalized_incoming_queue_evicts_oldest_and_retains_shared_order() {
    let _serial = audio_test_guard();
    clear_audio_contract_state();

    let session_id = "audio-contract-incoming";
    let evicted_before = evicted_pending_utterance_count();
    reset_finalized_meeting_sequence(session_id);
    reset_finalized_incoming_utterance_producer(session_id, TARGET_SAMPLE_RATE_HZ);

    finalize_incoming_fixture();
    finalize_incoming_fixture();
    finalize_incoming_fixture();

    assert!(
        try_take_finalized_incoming_utterance("different-session").is_none(),
        "a mismatched session must not consume retained incoming work"
    );

    let second = try_take_finalized_incoming_utterance(session_id)
        .expect("second finalized utterance should remain after oldest eviction");
    let third = try_take_finalized_incoming_utterance(session_id)
        .expect("third finalized utterance should remain after oldest eviction");

    assert_eq!(second.sequence, 2);
    assert_eq!(third.sequence, 3);
    assert_eq!(second.utterance_id, 2);
    assert_eq!(third.utterance_id, 3);
    assert_eq!(second.lane, "incoming");
    assert_eq!(third.lane, "incoming");
    assert!(second.generation.is_none());
    assert!(third.generation.is_none());
    assert_eq!(second.frame.sample_rate_hz, TARGET_SAMPLE_RATE_HZ);
    assert_eq!(third.frame.sample_rate_hz, TARGET_SAMPLE_RATE_HZ);
    assert_eq!(second.frame.channels, TARGET_CHANNELS);
    assert_eq!(third.frame.channels, TARGET_CHANNELS);
    assert!(try_take_finalized_incoming_utterance(session_id).is_none());
    assert_eq!(
        evicted_pending_utterance_count().saturating_sub(evicted_before),
        1,
        "the max-two pending queue should evict exactly the oldest third-backlog item"
    );

    clear_audio_contract_state();
}

#[test]
fn finalized_writer_rejects_lane_and_target_format_violations_before_staging() {
    let _serial = audio_test_guard();
    clear_audio_contract_state();

    let outbound_lane_mismatch = utterance_fixture(
        "incoming",
        Some(1),
        TARGET_SAMPLE_RATE_HZ,
        TARGET_CHANNELS,
        8_000,
    );
    let report = write_finalized_outbound_utterance_wav(&outbound_lane_mismatch);
    assert!(!report.ok);
    assert!(report.audio_path.is_none());
    assert_eq!(report.blocker, "finalized_outbound_writer:lane_mismatch");

    let incoming_generation_mismatch = utterance_fixture(
        "incoming",
        Some(1),
        TARGET_SAMPLE_RATE_HZ,
        TARGET_CHANNELS,
        8_000,
    );
    let report = write_finalized_incoming_utterance_wav(&incoming_generation_mismatch);
    assert!(!report.ok);
    assert!(report.audio_path.is_none());
    assert_eq!(report.blocker, "finalized_incoming_writer:lane_mismatch");

    let wrong_rate = utterance_fixture("you", Some(1), 8_000, 1, 4_800);
    let report = write_finalized_outbound_utterance_wav(&wrong_rate);
    assert!(!report.ok);
    assert!(report.audio_path.is_none());
    assert_eq!(
        report.blocker,
        "finalized_utterance_writer:frame_not_target_format"
    );

    let too_short = utterance_fixture(
        "you",
        Some(1),
        TARGET_SAMPLE_RATE_HZ,
        TARGET_CHANNELS,
        1_600,
    );
    let report = write_finalized_outbound_utterance_wav(&too_short);
    assert!(!report.ok);
    assert!(report.audio_path.is_none());
    assert_eq!(report.blocker, "finalized_utterance_writer:segment_too_short");

    let empty = utterance_fixture(
        "you",
        Some(1),
        TARGET_SAMPLE_RATE_HZ,
        TARGET_CHANNELS,
        0,
    );
    let report = write_finalized_outbound_utterance_wav(&empty);
    assert!(!report.ok);
    assert!(report.audio_path.is_none());
    assert_eq!(
        report.blocker,
        "finalized_utterance_writer:sample_count_out_of_range"
    );

    clear_audio_contract_state();
}

#[test]
fn stale_outbound_waiter_preserves_newer_generation_and_revocation_drops_matching_pending() {
    let _serial = audio_test_guard();
    clear_audio_contract_state();

    let stale_generation = begin_meeting_generation();
    let _ = clear_runtime_session_state();
    let current_generation = begin_meeting_generation();
    assert_ne!(stale_generation, current_generation);

    let session_id = "audio-contract-outbound-generation";
    reset_finalized_meeting_sequence(session_id);
    reset_finalized_outbound_utterance_producer(
        session_id,
        current_generation,
        TARGET_SAMPLE_RATE_HZ,
    );
    finalize_outbound_fixture();

    assert!(
        wait_take_finalized_outbound_utterance(stale_generation).is_none(),
        "a stale consumer must be rejected without touching a newer producer"
    );
    let current = wait_take_finalized_outbound_utterance(current_generation)
        .expect("newer authoritative producer must retain its pending utterance");
    assert_eq!(current.sequence, 1);
    assert_eq!(current.lane, "you");
    assert_eq!(current.generation, Some(current_generation));
    assert_eq!(current.frame.sample_rate_hz, TARGET_SAMPLE_RATE_HZ);
    assert_eq!(current.frame.channels, TARGET_CHANNELS);

    finalize_outbound_fixture();
    let _ = revoke_runtime_session_authority(
        current_generation,
        "audio contract revokes outbound authority before pending cleanup",
    );
    assert!(
        wait_take_finalized_outbound_utterance(current_generation).is_none(),
        "matching producer work must be discarded once its generation loses authority"
    );

    clear_audio_contract_state();
}

#[test]
fn finalized_source_rate_transition_resets_old_preroll_and_resamples_to_target() {
    let _serial = audio_test_guard();
    clear_audio_contract_state();

    let transition_session = "audio-contract-rate-transition";
    reset_finalized_meeting_sequence(transition_session);
    reset_finalized_incoming_utterance_producer(transition_session, 8_000);

    // Old-rate silence becomes pre-roll. Switching to 16 kHz must clear that
    // pre-roll and the old boundary state before accepting the new-rate speech.
    observe_finalized_incoming_f32_samples(&silence_of(8_000, 200), 8_000, 1);
    observe_finalized_incoming_f32_samples(&speech_of(16_000, 400), 16_000, 1);
    observe_finalized_incoming_f32_samples(&silence_of(16_000, 400), 16_000, 1);

    let transitioned = try_take_finalized_incoming_utterance(transition_session)
        .expect("new-rate speech should finalize after the source-rate reset");
    assert_eq!(transitioned.frame.sample_rate_hz, TARGET_SAMPLE_RATE_HZ);
    assert_eq!(transitioned.frame.channels, TARGET_CHANNELS);
    assert_eq!(
        transitioned.frame.samples.len(),
        12_800,
        "old 8 kHz pre-roll must not leak into the 16 kHz utterance"
    );
    assert!(transitioned
        .frame
        .samples
        .iter()
        .all(|sample| sample.is_finite() && (-1.0..=1.0).contains(sample)));

    clear_finalized_incoming_utterance_producer();
    clear_finalized_meeting_sequence();

    let resample_session = "audio-contract-resample";
    reset_finalized_meeting_sequence(resample_session);
    reset_finalized_incoming_utterance_producer(resample_session, 8_000);
    observe_finalized_incoming_f32_samples(&speech_of(8_000, 400), 8_000, 1);
    observe_finalized_incoming_f32_samples(&silence_of(8_000, 400), 8_000, 1);

    let resampled = try_take_finalized_incoming_utterance(resample_session)
        .expect("8 kHz finalized speech should be resampled to the target format");
    assert_eq!(resampled.frame.sample_rate_hz, TARGET_SAMPLE_RATE_HZ);
    assert_eq!(resampled.frame.channels, TARGET_CHANNELS);
    assert_eq!(resampled.frame.samples.len(), 12_800);
    assert!(resampled
        .frame
        .samples
        .iter()
        .all(|sample| sample.is_finite() && (-1.0..=1.0).contains(sample)));

    clear_audio_contract_state();
}
