# RFC — Meeting Quality-First Adaptive Streaming (RTX 3070 / i7-14700 / 32 GB)

**Status:** PROPOSED ARCHITECTURE; instrumentation-only source candidate started 2026-10-12, target Windows runtime not proven. No model/streaming-mode changes authorized by this RFC.
**Source baseline:** `a89e9501d68dc30082a332bd37be8e2a5e85f01d` on `Local`, 2026-10-12.
**Goal:** deliver speech translation for online meetings with substantially less perceived latency *without regression in meaning, voice quality, audio routing, or long-session reliability*.
**Hardware constraint:** one dedicated NVIDIA RTX 3070 (8 GB VRAM) shared by desktop/browser and local inference; Intel Core i7-14700 class CPU (20C/28T); 32 GB system RAM; no usable iGPU. Hardware identity and runtime precision need target-Windows verification.

## 1. Findings confirmed in source (not target-Windows timings)

- `src-tauri/src/commands/helper_bridge_runtime/scheduler.rs` gates the canonical helper with one active request at a time; priority is outbound > incoming > Text > diagnostics, but an already-active job is nonpreemptive.
- Current outbound consumes **finalized utterances** and calls ASR, MiLMMT translation, and GPT-SoVITS synthesis sequentially. `vad.rs` defines `partial_asr_enabled: true` but production consumer does not perform inference on draft audio.
- ASR uses Faster Whisper large-v3-turbo (`int8_float16` on CUDA); MiLMMT uses BF16 on CUDA when supported; GPT-SoVITS voice inference uses half precision on CUDA and `streaming_mode=False`, `return_fragment=False`.
- Current TTS creates one completed WAV before playback; CPAL creates, plays, and closes a native stream for each WAV. Persistent output is **not** a proven improvement until native tests.
- Existing `outbound_latency_ms` starts at **utterance finalization**, not first spoken phoneme. End-to-end user delay is not yet measured.
- Finalized producer has two pending utterance slots and drops overlong >60s unbroken utterances. Incoming deferred queue is four items, stale after 20s. Avoid treating any of those safety limits as the desired product speech-turn policy.
- Real Google Meet/Chrome route reception, overlapping speech suppression, and 30-minute native endurance have not been accepted on this SHA. Current source/old benchmarks cannot prove performance on this hardware.

## 2. Operational principles

1. Keep user-facing control **one Start/Stop button**. Do not add manual streaming/quality/performance switches.
2. A single canonical Meeting/session/generation owner protects output and Stop. Every draft, committed unit, synthesis chunk and output carries session ID, generation and monotonically increasing sequence.
3. Stage outputs are immutable after **commit**. Draft ASR can be revised; audio already played can never be silently rolled back.
4. Preserve critical meaning (especially negation, numbers, names, dates, code-switching) ahead of speed. No regex/heuristic can truthfully guarantee semantic equivalence.
5. No auto CPU fallback or model unload/reload during a live meeting merely to avoid visible OOM; use truthful blocked/degraded status and controlled explicit recovery.
6. Incoming may degrade independently; never sacrifice outbound or misidentify mixed Windows output as distinct participants.
7. Private speech, transcripts, model prompts and raw audio must not be committed to diagnostics/GitHub; only redacted performance aggregates and source fingerprints.

## 3. Architecture boundaries to prototype

```text
physical mic -> CPAL capture -> cheap CPU VAD/ring buffer
                         |                  |
                         |                 draft audio snapshot (admission guarded)
                         v                  v
                   final ASR          optional partial ASR
                         |                  |
                         +-------- stable transcript owner (revision, overlap dedupe)
                                             |
                                       semantic-safe segments
                                             |
                                   canonical MiLMMT requests
                                             |
                                   committed translation queue
                                             |
                          canonical GPT-SoVITS voice synthesis
                                             |
                       persistent/bounded PCM output candidate
                                             |
                         authorized virtual mic -> browser call
```

Keep finalized-utterance pipeline as control baseline. Streaming component boundaries **must not** duplicate full model runtimes or separate Meeting state owners. Record source revision and stable segment ranges so overlapping ASR windows do not duplicate words.

### Inference admission / scheduling

- CPU captures audio continuously into bounded preallocated structures; callbacks never block on Python/model/disk operations.
- Draft ASR is *optional*; no high-frequency 100-ms inference loop. Use adaptive, coalesced windows with overlap only when a speech segment is long, queue is idle, and no higher-priority outbound translation/TTS work waits.
- **At most one outstanding draft ASR request** per lane. Replace superseded unstarted drafts, invalidate completion by revision/session, and skip drafts under GPU pressure.
- Do not execute ASR/translation/TTS simultaneously on CUDA by default. One AI GPU permit is the safe baseline until measured resident-model footprint and contention establish otherwise.
- Do not demote official outbound TTS behind speculative ASR. Scheduler wait time, queue age, worker occupancy and freshness should be measured separately.
- A *soft freshness limit* for queued voice outputs must be configurable internally from baseline data; stale outputs are rejected and visibly accounted, never silently spoken minutes later.

### Commit policy

`DRAFT_ASR -> STABLE_ASR -> TRANSLATION_CANDIDATE -> VERIFIED_COMMIT -> TTS_READY -> PLAYING -> PLAYED/FAILED/INTERRUPTED`.

Stable ASR requires corroborated local observations and coherent boundaries, not merely elapsed milliseconds. Before audio commit, apply deterministic checks for empty/prompt echo/corrupt output, direction mismatch, severe omission signals and high-risk negation/number/entity changes where verifiable. Uncertain content should remain pending or report failure, not be guessed aloud. Preserve original recognized text even when translation fails.

### Output transport

- First benchmark the current per-WAV CPAL creation cost with real samples.
- Prototype **one generation-bound persistent CPAL stream** fed by a bounded PCM ring buffer; underflow emits silence (not recycled speech), Stop revokes output immediately, and callbacks do not allocate/block.
- Preserve ordering, at-most-once playback, cancellation, device-loss handling, per-session cleanup and current virtual microphone isolation.
- Investigate GPT-SoVITS fragment synthesis separately; only enable it if voice identity, intonation, joining artifacts, GPU cost and latency all beat completed-WAV baseline. No automatic flag flip to `streaming_mode=True`.

## 4. Measurement schema — compare same exact corpus/device/browser condition

Per utterance store **numeric, redacted**:
- capture/speech onset and speech offset (monotonic clocks); VAD boundary/finalization; draft accepted/discarded; commit timestamps
- ASR GPU wait+inference, MiLMMT wait+inference, TTS wait+inference, playback enqueue and native first-audio callback
- onset-to-first-audio, offset-to-first-audio and finalized-to-first-audio; P50/P95/P99 and worst case
- dropped/missed segments by cause, stale outputs, queue depth, inference occupancy, device callbacks/underruns
- Windows shared GPU peak VRAM and allocator peak VRAM, CPU/RAM, GPU utilization, browser GPU load, run temperature/clock throttling
- semantic critical errors and human-audited sample outputs (separate private evaluator evidence from diagnostics)

`outbound_latency_ms` in current source remains a *post-finalization* measurement and must not be presented as full real-time experienced latency.

## 5. Experiment sequence (one change domain at a time)

**P0: No-change baseline.** Build exact SHA first; run real Windows Google Meet/receiver on same physical microphone/voice, warm and cold models, typical utterances including negation, short replies and 60+s uninterrupted speech, two-way audio overlap, five Start/Stop, thirty-minute endurance. Observe first wrong owner. Record memory and timings without private audio leakage.

**P1: No-model optimization.** Benchmark persistent audio output against per-WAV output; cache model artifacts; reduce avoidable resampling/file/stream overhead only if measurable, preserve native audio ownership.

**P2: Controlled partial ASR.** Add one admission-guarded preview window while speech continues. No TTS/translation from uncommitted draft. Compare GPU scheduled preview versus CPU preview only if memory/time justify CPU use; hybrid CPU does not imply CPU decoding is cheap.

**P3: Clause-aware incremental translation.** Commit only stable meaningful spans (never naked negation/pronoun/context fragment); use overlap dedupe and coherent-order tests. Full source context should still determine how segments are translated, but never inject previous *model translations* as autonomous examples.

**P4: Optional TTS streaming.** Trial fragment delivery in isolated evaluator before production; if voice quality worsens, retain complete-phrase synthesis plus better buffering.

**P5: Integrated browser/30-minute test.** Exercise long monologues, rapid turns, overlapping participants, lost/changed audio device, VAD short utterances, stop during each pipeline stage, GPU memory pressure, browser background tab and transcript export.

Each stage is behind a developer-only acceptance mechanism, not a new product switch. Avoid keeping multiple unmaintained permanent execution engines.

## 6. Candidate promotion criteria (proposed, not observed)

- Measured onset-to-first-audio P50 improves by ≥15% in matched real meeting scenarios; offset-to-first-audio improves or stays equal.
- **P95 must not regress**, and very late/stale speech must not be played as though current.
- Zero newly introduced critical meaning errors on the approved regression corpus, with explicit human review of negation, dates, numbers, technical terms, code-switching and long utterances.
- No increase in naturally spoken dropped phrases, TTS repeats, callback errors, noise feedback or wrong-device output.
- VRAM remains below sustainable Windows WDDM budget with measured reserve. *Initial planning preference* ≥~1 GB shared-GPU headroom; not a hard safe threshold before profiling. No OOM / unintended host spilling / heavy page-thrashing.
- 30-minute browser meeting + five Start/Stop cycles without rising GPU/RAM use, stale audio after Stop, or degraded primary outbound.
- If a candidate fails, leave existing final-utterance baseline unchanged.

## 7. Implementation order and files

- **Metrics:** `src-tauri/src/engine/audio/finalized_utterance.rs`, `commands/meeting_session/session_state.rs`, existing diagnostic/performance runbook.
- **Output:** `src-tauri/src/engine/audio/meeting_output_runtime.rs`, `commands/meeting_session/playback_runtime.rs` (native generation-bounded cancellation tests).
- **Draft:** `src-tauri/src/engine/audio/finalized_utterance.rs` + `commands/meeting_session/consumer_runtime.rs`, canonical helper scheduler, worker ASR request path.
- **Translation:** `commands/meeting_session/outbound_pipeline.rs`, canonical MiLMMT worker; use existing translation quality corpus/human rubric.
- **TTS:** `WorkerRuntime/voice_lab_gpt_sovits.py` + Rust playback, separate A/B audio-quality listening test.
- **Evidence:** `docs/knowledge/operations/target-windows-performance.md`, acceptance scenarios Group B/C and quality evaluator reports, not a second persistent results platform.

## 8. Non-goals

No Gemini/other cloud API, no mandatory new model, no speaker diarization, no speculative second GPU worker, no immediate complete model replacement, no UI settings for streaming modes, no automatic permanent transcript recording, no claim that source checks prove Google Meet delivery.

**Next action:** run exact-SHA source/build admission for the instrumentation candidate, then one authorized native baseline (latency/VRAM/browser). Only if measured output-stream construction materially contributes to delays, compare a generation-bound persistent-stream prototype. Streaming ASR/TTS remains gated, not assumed.
