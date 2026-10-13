# M-TranslateIT — Meeting V2 implementation plan (source-first)

**Document role:** IMPLEMENTATION BLUEPRINT ONLY. These are candidate stages and proof criteria, not current tasks or evidence of completed execution. The active continuation is owned solely by [development.md](./development.md); the source proof ceiling is owned by [Current Validation](../docs/knowledge/current-validation.md).
**Design origin:** `75cd24e4f52708ec51105bf9cb2beed4833e7773` (2026-10-12); historical authoring reference only, not the current `Local` HEAD or a verified runtime baseline.
**Primary goal:** natural, accurate, low-latency bilingual browser meetings on RTX 3070 8 GB, i7-14700-class CPU, 32 GB system RAM, **no available iGPU**.
**Execution boundary:** Source proposals can be reviewed on GitHub `Local`; compilation, CUDA, native audio, and browser behavior require one later integrated local acceptance. Hosted CI/builds are inactive.
**Related RFC:** [Meeting Quality-First Adaptive Streaming](../docs/knowledge/proposals/2026-10-12-meeting-quality-adaptive-streaming.md). This plan is its implementation order and does not authorize premature replacement of production inference.

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

## 2. Candidate stages (not active work orders)

### Batch 0 — Canonical source contracts and cleanup
- Preserve current final-utterance path as control and capture immutable exact-SHA baseline.
- Preserve the style-free incoming request contract; do not reintroduce retired `translation_style` routing or alter direction/session semantics.
- Keep the VAD runtime profile free of dormant partial-ASR enablement metadata. Only a separately reviewed, measured and verified draft-ASR inference owner may admit partial work; metadata alone cannot activate it.
- Unify status vocabulary and worker stage identity; assert no stopped/stale session can publish audio.
- Fix conflicts between previous decisions, manual test runbooks, and retired features.
- **Proof:** Rust/Python/TS unit source tests and full compiler checks in a matching environment; no target-latency claims.

### Batch 1 — Reliable capture and full-turn records

- Replace blocking callback-side state manipulation with a bounded, preallocated capture handoff designed for audio thread deadlines. Track queue-full and OS callback errors; don't silently drop.
- Carry monotonic capture time and sample offset through segmentation, ASR, translation and first-audio events. Distinguish onset estimate from observed audio frame boundaries.
- Preserve stable source transcript on translation/TTS failure; UI shows failed translation without fabricating it.
- Rework retention: view the last 240 entries, but hold the complete session in bounded memory for explicit manual export, with session transitions and cap warnings.
- **Proof:** replay identical synthetic timing and failures, no dropped/duplicated words in basic fixtures; buffer stress, 60+s monologue, concurrent Start/Stop, session reentry and overflow tests.

### Batch 2 — CPU/ring-buffer playback candidate

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

### Batch 6 — Integrated local and target-Windows evidence gate
- When an authorized local code/toolchain context becomes available, run **existing** compile/typecheck/pytest/Rust/native-independent tests on an exact clean `Local` SHA. GitHub Code Health workflows are inactive; no hosted execution or dispatch.
- Produce one pinned-SHA run plan rather than per-commit PC tasks, with private/test-safe fixtures, expected metrics, and no audio contents in support bundles.
- When PC becomes available, validate RTX 3070 + Chrome/Meet concurrent VRAM + same receiver, actual output latency P50/P95, cold/warm model residency, 30-minute session and 5 Start/Stop cycles. Only then promote best candidate.
- **Proof ceiling:** source-only changes remain `SOURCE_CANDIDATE`; executed source checks establish only their matching local-code scope, and `TARGET_WINDOWS_ACCEPTED` requires real GPU/audio/browser evidence. Hosted CI is not part of this gate.

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

## 5. Navigation and historical source candidates

This blueprint intentionally has **no separate Next step**. The only active continuation is [Active Development](./development.md), and the acceptance procedure is [Target Windows Performance Acceptance](../docs/knowledge/operations/target-windows-performance.md). Prior dated Batch A/B/C implementation notes are revision-specific historical evidence in `Local` commit history, not instructions to resume or claims of executed tests. Source-only `#[cfg(test)]` prototypes are not production streaming.
