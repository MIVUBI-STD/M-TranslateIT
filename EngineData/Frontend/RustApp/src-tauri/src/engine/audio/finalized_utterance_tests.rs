use super::*;

// Both scenarios run inside one test fn because the producer state lives in
// process-wide statics; parallel unit tests would race on those mutexes.

#[test]
fn incoming_lane_segmentation_eviction_and_overflow_are_observable() {
    // Scenario 1: three finalized utterances against the bounded pending
    // queue. The oldest must be evicted (observable counter) instead of
    // growing an unbounded realtime backlog.
    clear_finalized_meeting_sequence();
    clear_finalized_incoming_utterance_producer();
    reset_finalized_meeting_sequence("sess-evict");
    reset_finalized_incoming_utterance_producer("sess-evict", 16_000);

    let evicted_before = evicted_pending_utterance_count();
    let overflow_before = overflow_dropped_utterance_count();

    let mut speech = Vec::new();
    for i in 0..(16_000 * 300 / 1000) {
        speech.push(0.3 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 16_000.0).sin());
    }
    let silence = vec![0.0f32; 16_000 * 400 / 1000];

    for _ in 0..3 {
        observe_finalized_incoming_f32_samples(&speech, 16_000, 1);
        observe_finalized_incoming_f32_samples(&silence, 16_000, 1);
    }

    assert_eq!(evicted_pending_utterance_count() - evicted_before, 1, "oldest finalized utterance must be evicted once the third one arrives");
    assert_eq!(overflow_dropped_utterance_count() - overflow_before, 0);

    clear_finalized_incoming_utterance_producer();
    clear_finalized_meeting_sequence();

    // Scenario 2: continuous speech longer than the 60 s in-progress safety
    // bound is discarded and reported through the overflow counter rather
    // than forced into a fake boundary.
    clear_finalized_meeting_sequence();
    clear_finalized_incoming_utterance_producer();
    reset_finalized_meeting_sequence("sess-overflow");
    reset_finalized_incoming_utterance_producer("sess-overflow", 8_000);

    let evicted_before = evicted_pending_utterance_count();
    let overflow_before = overflow_dropped_utterance_count();

    let chunk: Vec<f32> = (0..(8_000 * 600 / 1000))
        .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 8_000.0).sin())
        .collect();
    // 101 chunks x 600 ms = 60.6 s of unbroken speech > 60 s ceiling.
    for _ in 0..101 {
        observe_finalized_incoming_f32_samples(&chunk, 8_000, 1);
    }
    observe_finalized_incoming_f32_samples(&silence_of(8_000, 400), 8_000, 1);

    assert_eq!(overflow_dropped_utterance_count() - overflow_before, 1, "overlong speech must land in the overflow counter");
    assert_eq!(evicted_pending_utterance_count() - evicted_before, 0);

    clear_finalized_incoming_utterance_producer();
    clear_finalized_meeting_sequence();
}

#[test]
fn preroll_bound_resample_and_safe_retention_are_deterministic() {
    let profile = runtime_vad_profile();
    let mut state = FinalizedProducerState {
        session_id: "pure-contract".to_string(),
        generation: None,
        lane: LANE_INCOMING,
        sample_rate_hz: 16_000,
        profile,
        pre_roll: VecDeque::new(),
        in_utterance: false,
        current_samples: Vec::new(),
        speech_samples: 0,
        trailing_silence_samples: 0,
        overflowed: false,
        next_utterance_id: 1,
        pending: VecDeque::new(),
    };
    let max_pre_roll = samples_for_duration(
        state.sample_rate_hz,
        state.profile.pre_roll_audio_ms,
    );
    append_pre_roll(&mut state, &vec![0.0; max_pre_roll.saturating_add(16_000)]);
    assert_eq!(state.pre_roll.len(), max_pre_roll);

    let resampled = resample_linear(&[0.0, 0.25, -0.25, 0.0], 8_000, 16_000);
    assert_eq!(resampled.len(), 8);
    assert!(resampled.iter().all(|sample| (-1.0..=1.0).contains(sample)));
    assert_eq!(safe_sample(f32::NAN), 0.0);
    assert_eq!(safe_sample(f32::INFINITY), 0.0);
    assert_eq!(safe_sample(2.0), 1.0);

    let mut retained = Vec::new();
    extend_safe_samples(&mut retained, &[f32::NAN, 2.0, -2.0, 0.25]);
    assert_eq!(retained, vec![0.0, 1.0, -1.0, 0.25]);

    let i16_mono = convert_and_downmix(&[i16::MAX, 0, i16::MIN], 1, |sample| {
        (sample as f32 / i16::MAX as f32).clamp(-1.0, 1.0)
    });
    assert_eq!(i16_mono, vec![1.0, 0.0, -1.0]);

    let stereo = convert_and_downmix(&[i16::MAX, 0, 0, i16::MIN], 2, |sample| {
        (sample as f32 / i16::MAX as f32).clamp(-1.0, 1.0)
    });
    assert_eq!(stereo.len(), 2);
    assert!((stereo[0] - 0.5).abs() < 0.0001);
    assert!((stereo[1] + 0.5).abs() < 0.0001);
}

#[test]
fn clipped_burst_during_active_speech_does_not_become_a_silence_boundary() {
    // Pure producer state: no global Meeting generation or audio device needed.
    let mut state = FinalizedProducerState {
        session_id: "clipping-boundary".to_string(),
        generation: None,
        lane: LANE_INCOMING,
        sample_rate_hz: 16_000,
        profile: runtime_vad_profile(),
        pre_roll: VecDeque::new(),
        in_utterance: false,
        current_samples: Vec::new(),
        speech_samples: 0,
        trailing_silence_samples: 0,
        overflowed: false,
        next_utterance_id: 1,
        pending: VecDeque::new(),
    };
    let clear_speech = vec![0.12_f32; 16_000 / 4];
    let mut clipped_burst = clear_speech.clone();
    for sample in clipped_burst.iter_mut().step_by(20) {
        *sample = 1.0;
    }
    let clear_evidence = AudioEvidenceReport::from_samples(&clear_speech);
    let clear_gate = evaluate_vad_gate(clear_evidence.clone(), &state.profile.gate);
    assert!(clear_gate.accepted);
    assert!(!ingest_observation(
        &mut state,
        &clear_speech,
        &clear_evidence,
        &clear_gate,
    ));
    assert!(state.in_utterance);

    let clipped_evidence = AudioEvidenceReport::from_samples(&clipped_burst);
    let clipped_gate = evaluate_vad_gate(clipped_evidence.clone(), &state.profile.gate);
    assert_eq!(clipped_gate.reason, "rejected_clipping");
    // 250 ms exceeds the current adaptive silence interval, but it is
    // distorted speech-like energy, not a natural end-of-speech pause.
    assert!(!ingest_observation(
        &mut state,
        &clipped_burst,
        &clipped_evidence,
        &clipped_gate,
    ));
    assert!(state.in_utterance);
    assert_eq!(state.trailing_silence_samples, 0);
    assert_eq!(state.speech_samples, clear_speech.len());

    assert!(!ingest_observation(
        &mut state,
        &clear_speech,
        &clear_evidence,
        &clear_gate,
    ));
    assert_eq!(state.speech_samples, clear_speech.len() * 2);
    assert_eq!(
        state.current_samples.len(),
        clear_speech.len() * 2 + clipped_burst.len()
    );
    assert!(state.pending.is_empty());
}

#[test]
fn incoming_bounded_wait_distinguishes_deferred_retry_from_stopped_producer() {
    let sync = FinalizedProducerSync {
        state: Mutex::new(Some(FinalizedProducerState {
            session_id: "retry-session".to_string(),
            generation: None,
            lane: LANE_INCOMING,
            sample_rate_hz: 16_000,
            profile: runtime_vad_profile(),
            pre_roll: VecDeque::new(),
            in_utterance: false,
            current_samples: Vec::new(),
            speech_samples: 0,
            trailing_silence_samples: 0,
            overflowed: false,
            next_utterance_id: 1,
            pending: VecDeque::new(),
        })),
        ready: Condvar::new(),
    };
    assert!(matches!(
        wait_take_finalized_incoming_utterance_for_sync(
            &sync, "retry-session", Duration::from_millis(1)
        ),
        IncomingTimedWait::TimedOut
    ));
    assert!(matches!(
        wait_take_finalized_incoming_utterance_for_sync(
            &sync, "another-session", Duration::from_millis(1)
        ),
        IncomingTimedWait::Stopped
    ));

    sync.state.lock().unwrap().as_mut().unwrap().pending.push_back(
        FinalizedMeetingUtterance {
            session_id: "retry-session".to_string(),
            sequence: 1,
            lane: LANE_INCOMING.to_string(),
            generation: None,
            utterance_id: 1,
            finalized_at: Instant::now(),
            enqueued_at: Instant::now(),
            finalized_unix_ms: 0,
            speech_boundary_ms: 100,
            speech_duration_ms: 300,
            finalization_ms: 0,
            frame: AudioFrame {
                sample_rate_hz: TARGET_SAMPLE_RATE_HZ,
                channels: TARGET_CHANNELS,
                samples: vec![0.1; 4_800],
            },
        },
    );
    assert!(matches!(
        wait_take_finalized_incoming_utterance_for_sync(
            &sync, "retry-session", Duration::from_millis(1)
        ),
        IncomingTimedWait::Utterance(utterance) if utterance.sequence == 1
    ));
    *sync.state.lock().unwrap() = None;
    sync.ready.notify_all();
    assert!(matches!(
        wait_take_finalized_incoming_utterance_for_sync(
            &sync, "retry-session", Duration::from_millis(1)
        ),
        IncomingTimedWait::Stopped
    ));
}

fn silence_of(rate: u32, ms: u32) -> Vec<f32> {
    vec![0.0f32; rate as usize * ms as usize / 1000]
}
