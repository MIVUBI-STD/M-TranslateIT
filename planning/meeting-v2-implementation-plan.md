# M-TranslateIT — Meeting V2 implementation plan (source-first)

**Status:** IMPLEMENTATION BLUEPRINT. Batch 0 cleanup and Batch 1 source-first/long-session bounded retention have source candidates on `Local`; the compiler and GPU/browser acceptance have not run. More audio capture/streaming batches remain pending.
**Baseline:** `75cd24e4f52708ec51105bf9cb2beed4833e7773` on `Local` (2026-10-12).
**Primary goal:** natural, accurate, low-latency bilingual browser meetings on RTX 3070 8 GB, i7-14700-class CPU, 32 GB system RAM, **no available iGPU**.
**Current execution context:** GitHub source review/writes only. Prepare modular source and deterministic tests; compiler/CUDA/Windows/browser tests remain unexecuted until an integrated local acceptance phase. No hosted CI or cloud builds.
**Related RFC:** `docs/knowledge/proposals/2026-10-12-meeting-quality-adaptive-streaming.md`. This plan is its implementation order and does not authorize premature replacement of production inference.

**Target experience (not an equivalence claim):** continuous ID→EN local meeting speech with earlier first intelligible audio, stable meaning, and natural selected voice on RTX 3070 8 GB. Retain the single Tauri/Rust Meeting owner, single Python worker, one GPU permit and existing Whisper → MiLMMT → GPT-SoVITS models. Reuse existing VAD and playback owners rather than creating a V3 engine, provider registry, always-on background service, or new UI controls. Outbound comes first; incoming EN→ID remains optional text only. Streaming is a candidate behind exact-SHA compiler, native audio, semantic-quality and 30-minute stability gates—not an automatic product switch.

**Commit rule for future streaming:** today's production output still requires a finalized utterance. Progressive sub-utterance commits may replace that boundary only after stable-prefix overlap/revision tests, human meaning review (especially negation/numbers), no duplicate/late speech, and measured end-to-end gains. Draft ASR is replaceable and optional under GPU pressure; synthesized/playable audio is immutable and at most once. All chunks remain tied to the canonical session, lane, generation, utterance and source span. No arbitrary fixed-time splitting.

**Hardware admission:** CPU owns bounded capture/VAD/buffer management; a single shared 8 GB GPU inference permit runs priority-controlled ASR, translation and voice. Simultaneous model residency is conditional on measured peak VRAM with browser open; never assume that weights fitting implies runtime headroom. No automatic CPU swap/unload mid-meeting. Benchmark matched P50/P95 onset/end-to-first-audio, meaning fidelity, packet loss and audible output at the receiving meeting app, not just model-step speed.

## 0. Architectural hard rules

1. There is exactly one canonical Meeting session/generation owner; only it may commit, play, retry or stop. Do not build a parallel Meeting engine.
2. Audio I/O callbacks must be **bounded, non-blocking and allocation-free as practicable**: never block on a scheduler mutex, disk access, Python, or model inference. Record overflow instead of blocking. CPAL source currently calls `finalized_utterance::observe_finalized_mono_samples`, which takes a `Mutex` and can grow `Vec` or create downmixed buffers; audit this before introducing draft streaming.
3. One GPU worker permit is the default on a shared 8 GB RTX 3070. Existing helper model residency and stage priority are retained until actual GPU/VRAM evidence supports a change. Do not add speculative GPU workers.
4. Internal stage states: `CAPTURED -> ASR_DRAFT -> ASR_STABLE -> TRANSLATION_PENDING -> TRANSLATION_COMMITTED -> TTS_READY -> PLAYING -> PLAYED`; explicit `FAILED`, `DROPPED`, `SUPERSEDED` and `INTERRUPTED` exits. Not all utterances must go through drafts. A first-pass finalized ASR goes directly to stable.
5. Drafts are replaceable, committed text and emitted audio are not. Strict `session_id / generation / lane / utterance_id / revision / monotonic sequence / source-span` identity checks on every asynchronous result.
6. Translation quality takes precedence: no prompt echoes, fabricated words, incorrect direction, unsafe omission or critical negation/number/entity change should be delivered as accepted speech. Deterministic checks catch known failures but cannot guarantee semantic correctness; use the existing translation corpus and human quality gate.
7. `Person 1` labels the **mixed incoming Windows output**, never a diarized participant. Incoming may fail separately, never stop otherwise healthy outbound. Microphone device and Meet virtual-mic route remain separate.
8. The user sees only Start/Stop Meeting, a truthful status and chat-style bilingual transcripts. Do not create streaming/preset/model/style/device controls on Meeting or reactivate retired features.
9. Transcript permanent export remains manual. Separate full in-memory session transcript from a 240-turn rendered window; any bounded full-session memory cap and incomplete result must be explicit. Preserve ASR source text when translation/TTS fails, with an honest status.
10. No real-time quality/latency/VRAM claims can pass solely from source checks or a hosted runner without the dedicated target machine.

## 1. Proposed module boundaries (minimal additions)

| Owner | Role | Files to extend/refactor |
|---|---|---|
| Audio ingest | timestamped bounded SPSC-style PCM queue, callback-safe accounting, per-lane capture | `engine/audio/finalized_utterance.rs`, microphone/meeting sound capture owner |
| Segment owner | natural VAD boundary and long-utterance safe cuts, per-segment overlap/offset | `engine/audio/vad.rs` and an internal segment coordinator |
| ASR stabilizer | only one coalesced optional draft request; superseded revisions never commit; stable prefix deduped | `meeting_session/consumer_runtime.rs` and existing worker ASR |
| Scheduler | one GPU permit, prioritize committed outbound TTS/translation over speculative drafts, record queue age/rejection | `helper_bridge_runtime/scheduler.rs`, `helper_bridge/transport.rs` |
| Translation | one canonical MiLMMT worker, clause-aware commit based on actually stable source, no previous generated translations in prompt | `meeting_session/outbound_pipeline.rs` and pinned worker |
| Synthesis | existing voice identity, completed WAV baseline, optionally fragment test adapter with same output contract | `WorkerRuntime/voice_lab_gpt_sovits.py` |
| Playback | bounded per-generation PCM queue and candidate persistent CPAL stream, at-most-once and immediate Stop | `engine/audio/meeting_output_runtime.rs`, `meeting_session/playback_runtime.rs` |
| Transcript | immutable source ASR and separate translation/playback statuses, full current-session bounded in-memory retention, manual export | `meeting_session/committed_turns.rs`, `transcript_export.rs`, chat components |
| Diagnostics | derived monotonic onset/end/finalization/audio callback metrics, scheduler wait, output underruns, redacted numeric support bundle | existing Meeting timing + Diagnostics |
| Verification | deterministic event/PCM replay, isolated fault injection, existing evaluators, matching local source compilation | existing Rust/Python/TS suites and `tools/*_quality` |

**Do not create a second model scheduler, a second backend service, a new test-results platform, or an always-on telemetry daemon.**

## 2. Batch execution (no PC dependency unless identified)

### Batch 0 — Canonical source contracts and cleanup
- Preserve current final-utterance path as control and capture immutable exact-SHA baseline.
- Remove inert `translation_style` from incoming deferred queue/pipeline; the worker request is already style-free. Do not alter direction/session semantics.
- Remove or clearly mark misleading dormant `partial_asr_enabled` behavior; don't claim partial ASR operational from config.
- Unify status vocabulary and worker stage identity; assert no stopped/stale session can publish audio.
- Fix conflicts between previous decisions, manual test runbooks, and retired features.
- **Proof:** Rust/Python/TS unit source tests and full compiler checks in a matching environment; no target-latency claims.

### Batch 1 — Reliable capture and full-turn records

**Batch 2B.2 source continuation (2026-10-12):** `pcm_handoff_replay.rs` is a **test-only CPU worker replay adapter** that drives the canonical finalized incoming and outgoing VAD from bounded handoff chunks. Incoming discontinuities reset the existing incoming speech boundary; outbound replay uses a generation-gated, `#[cfg(test)]` boundary reset. Four Rust tests cover natural incoming finalization, gap-driven reset without phantom transcript continuity, rejection of foreign/stopped generation, and authoritative outbound finalization. The buffer prototype also re-checks loss after the CPU drain copy before exposing audio to VAD. No CPAL callback is switched, no GPU task or second segmenter is introduced. These tests are authored but **not executed** until Cargo is available.

**Batch 2B PCM proof addition (2026-10-12):** the existing fixed-capacity test-gated handoff uses `try_lock`, explicit discontinuity accounting, session/lane/generation binding and Stop erasure. The canonical-VAD replay now includes a test-only CPU consumer that revokes audio first, wakes its idle thread and attempts a deadline-bounded join; a stalled worker produces an explicit timeout and retains its join handle for retry. Two behavioral tests exercise canonical VAD delivery and controlled timeout/retry. The handoff now reserves one consumer atomically before thread spawn, rejects duplicate/after-Stop starts, rolls back failed-spawn reservation and erases poisoned-ring audio during Stop. **These Rust tests and Windows timings are not executed** and production CPAL remains unchanged. Timeout/Drop is not guaranteed thread termination: this candidate must not enter production until lifecycle and device-cutover gates pass.

- Replace blocking callback-side state manipulation with a bounded, preallocated capture handoff designed for audio thread deadlines. Track queue-full and OS callback errors; don't silently drop.
- Carry monotonic capture time and sample offset through segmentation, ASR, translation and first-audio events. Distinguish onset estimate from observed audio frame boundaries.
- Preserve stable source transcript on translation/TTS failure; UI shows failed translation without fabricating it.
- Rework retention: view the last 240 entries, but hold the complete session in bounded memory for explicit manual export, with session transitions and cap warnings.
- **Proof:** replay identical synthetic timing and failures, no dropped/duplicated words in basic fixtures; buffer stress, 60+s monologue, concurrent Start/Stop, session reentry and overflow tests.

### Batch 2 — CPU/ring-buffer playback candidate

**Progress 2026-10-12:** Batch 2A hot-path changes staged for native compilation: bounded pre-roll eviction, reusable incoming F32 stereo conversion scratch, and one suppression-boundary reset per suppression transition rather than per suppressed callback. All need real Rust tests before calling them verified. Capture lock isolation and persistent CPAL output remain future work.

- Measure CPAL per-WAV stream startup from existing timing first.
- Implement an **internal candidate** long-lived stream with bounded PCM chunks, underrun silence and one session owner. No disk/model change, no new UI toggle.
- Stop/cancel removes all old-generation chunks, no audio may leak after Stop. Device removal causes truthful recovery.
- **Proof:** deterministic fake PCM, 0/partial/multiple chunks, overflow/underrun, change of device, stale generation, 5x Start/Stop, callback-no-blocking audit. Keep existing WAV path as control until measured.

### Batch 3 — ASR draft and long-speech segmentation
- Do not call Whisper on every 100-ms capture chunk. Admit at most one coalesced draft for long speech when worker permit and resources are genuinely available; previews may be skipped without losing original audio.
- Stable-prefix agreement and overlap anchoring must avoid duplicated/omitted words when repeated windows are redecoded.
- Replace current >60s unbroken utterance loss with bounded progressive segments and memory-safe rolling audio. Segment boundaries preserve negation, question continuation and source time range.
- **Proof:** deterministic replay of silence, short reply, 1–5 minute monologue, code-switch, correction, rapid turn, overlapping windows and backlog. If partial is worse than finalized baseline, keep its internal admission off.

### Batch 4 — Incremental semantic translation
- Only committed, coherent ASR spans enter translation. Context may include **previous original source fragments from the same utterance** for disambiguation but never untrusted/generated previous translation examples.
- Treat partial negation, subject, numerals, dates and named entities conservatively. A heuristic guard alone cannot certify meaning; human comparison and existing evaluator remain required.
- Prevent stale draft responses from overwriting a newer utterance revision; translated spans are output once and in order.
- **Proof:** corpus cases for negation, questions, corrections, mixed language, numbers, names, phrase overlap; evaluate baseline vs candidate without invented scores.

### Batch 5 — TTS fragment candidate and integrated orchestration
- Keep `streaming_mode=False` as production baseline; experiment with upstream fragment delivery only behind internal test harness. Require verified voice identity/intonation and smooth joins.
- Coordinate audio chunks with monotonic playback sequence, bounded jitter buffer and cancel tokens. Do not infer receiver confirmation from local callback completion.
- **Proof:** source/runtime-comparable WAV fixtures, playback sequence tests, text-to-audio coverage and voice listening rubric. Do not turn it on for users without target hardware acceptance.

### Batch 6 — Pre-local-PC gate / handoff
- When an authorized local code/toolchain context becomes available, run **existing** compile/typecheck/pytest/Rust/native-independent tests on an exact clean `Local` SHA. GitHub Code Health workflows are inactive; no hosted execution or dispatch.
- Produce one pinned-SHA run plan rather than per-commit PC tasks, with private/test-safe fixtures, expected metrics, and no audio contents in support bundles.
- When PC becomes available, validate RTX 3070 + Chrome/Meet concurrent VRAM + same receiver, actual output latency P50/P95, cold/warm model residency, 30-minute session and 5 Start/Stop cycles. Only then promote best candidate.
- **Proof ceiling:** `SOURCE_CANDIDATE` until executed local compile, `LOCAL_CODE_VERIFIED` after matching CI, and `TARGET_WINDOWS_ACCEPTED` only after real GPU/browser tests.

## 3. Offline regression matrix (must be executable without RTX 3070)

- Audio: silence, clipping, multi-channel downmix, sample-rate drift, device changed while live, callback pressure.
- VAD: one-word reply; long non-stop speech; overlapping source windows; disappearing audio frames; 60+ second boundary.
- ASR: draft changes negation; repeated final suffix; hallucinated speech from silence; out-of-order response.
- Translation: prompt echo; incorrect direction; number/negative omission; partial clauses; code-switch; no response; timeout.
- TTS: empty WAV, invalid sample rate, fragment gap, duplicate fragment, mid-playback Stop, delayed old-generation fragment.
- Incoming: mixed Windows audio, outbound self-suppression, participant overlap, priority starvation, four-item deferred queue expiry.
- Session: Start twice; Stop at each stage; helper failure/restart; transcript manual export and next Meeting reset.
- Privacy: no raw audio, transcripts, paths or speaker identities in diagnostics, errors or tracked test assets.

Every case must specify **owner → input → expected state → observable failure code**. No dummy PASS based only on static marker text.

## 4. Proposed promotion criteria

- The best candidate must beat control on the matched user scenario; initial target ≥15% improvement for P50 onset-to-first-audio, **no P95 regression**.
- No newly introduced critical meaning errors; human judgments on negative/number/date/name and voice naturalness must be at least baseline.
- No increase in dropped/reordered speech or broken Stop, no VRAM OOM/unintended device migration.
- On RTX 3070, record VRAM shared with browser/Windows; do not assume that ASR+MiLMMT+GPT-SoVITS all fit simultaneously from model weights alone.
- If measured gains are absent, keep robust final-utterance pipeline and reject streaming complexity.

## 5. Next source change

**Batch C.1 test-only source eligibility (2026-10-13):** The pure interpreter policy now requires exact candidate scope/revision and an actual finalized whole-window capture span before `COMMIT_READY`. Nonfinal LocalAgreement text always `WAIT`, including apparently complete punctuation, because the current Whisper worker does not provide word alignment. A small set of Indonesian open-clause hints is advisory only; no classifier, second model, prompt-context injection, early TTS or streaming production activation. Five authored Rust behavior scenarios remain NOT EXECUTED. Next source prerequisite for progressive commits: real ASR-partial audio/word boundary evidence and revision-safe source claim.

**Batch B.5 source candidate (2026-10-13):** Production CPAL error closures now increment per-stream atomic markers; canonical VAD observes errors with its existing lock before/after accepted PCM ingest, invalidates unsafe source epochs, and binds markers only to the matching stream owner. Diagnostic logging is best-effort `try_lock`, while aggregate error counts remain atomic. No additional inference engine, production PCM handoff cutover, or streaming output. Existing callback mutex contention and device-dropped frames without a CPAL error remain unproven. Native/Cargo tests **NOT EXECUTED**. Next: bounded semantic commit decisions from source-aligned ASR observations.

**Batch B.4 scoped loss-to-epoch source candidate (2026-10-13):** The existing PCM replay resets only a matching incoming session or matching outbound session/generation VAD owner and advances that producer's accepted-mono-sample epoch after queue loss; a poisoned ring also triggers an invalidation callback. Session-bound self-output suppression uses the same owner check, preventing stale guards from clearing a newer Meeting session. The test-only replay checks output epoch isolation; no production PCM cutover, CUDA use, word alignment or partial translation is enabled. All authored Rust/native checks remain **NOT EXECUTED**. Next: connect real CPAL-known callback discontinuity to canonical VAD with safe nonblocking admission.

**Batch B.3 source candidate (2026-10-13):** The canonical finalized VAD producer now supplies producer-accepted mono input-clock epoch/native sample start-end and exact ASR frame length as optional metadata, including pre-roll, trailing silence, and native-to-16-kHz resampling. A test-only adapter converts it into the existing source-observation clock contract. It does NOT prove OS device timestamps, callback continuity before VAD, or any word alignment. The next boundary is explicit capture/handoff loss propagation into this epoch; streaming still disabled and all native/Rust tests NOT EXECUTED.

**Capture-clock mapping source candidate (2026-10-13):** Test-only source observations now accept contiguous producer-asserted ASR/capture window mapping, reject gaps and incompatible clocks, and map whole-window positions conservatively. Overlapping shifted windows create a new text anchor; nonfinal agreement never invents word timestamps. A final ASR result without capture provenance still has NO capture-aligned source span. The candidate is not wired to the real producer, VAD trimming or resampler; physical continuity and exact alignment remain unproved. The next source owner is the capture/audio-preparation boundary, not another ASR model or scheduler. Runtime acceptance remains deferred.

**Source observation continuation (2026-10-12):** A test-only bounded Rust ASR observation buffer implements same-anchor LocalAgreement, exact epoch/speech/revision rejection and explicit final-window vs text-only evidence. It neither retrieves word timestamps from Faster Whisper nor maps rolling ASR windows back to microphone samples. It is not a production streaming enablement or an executed test. Next source boundary: verified rolling-window alignment and ASR-domain/capture sample mapping before any progressive audio-span claim.

The source-only Batch A authority slice follows PCM Batch 2B: one-time turn admission, replay rejection after bounded transcript eviction, a locked-translation to queued transition, and exact identity playback claim. It does **not** implement sample-level SourceSpanId, audio epoch, ASR revision or progressive commits; the current finalized utterance remains the only production boundary. Next source batch: bounded source-span/revision ownership with ASR observations. All authored Rust tests, Windows GPU/audio and receiver checks remain **NOT EXECUTED** until integrated local acceptance. No hosted CI or unaccepted CPAL cutover.
