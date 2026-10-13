## Active continuation — Production Meeting Starting rollback safety (2026-10-13)

**Confirmed source defect:** `meeting_session/lifecycle.rs` previously ignored microphone, optional sound-capture, helper, outbound/playback consumer, suppression and transcript cleanup results during failed `Starting`, then unconditionally attempted to clear the Meeting session. Some Start failure messages unconditionally claimed "rolled back safely" although native cleanup was unknown.

**Bounded source fix:** Reuse the existing `meeting_cleanup_complete` and revoked-session `mark_runtime_session_cleanup_incomplete`/retry-Stop mechanisms. Starting rollback now checks output cancellation, both capture stops, helper cancellation, outbound/playback shutdown, suppression/transcript release and session-owned temporary WAV/TTS cleanup. It releases Meeting ownership only when every check passed and matching-generation clear was confirmed; otherwise it retains/blocks the previous generation, marks cleanup incomplete and reports retry-Stop truthfully. No new worker, manager, model, user mode or streaming behavior. One Rust regression test exercises the exact cleanup decision used in production, including each individual failed requirement.

**Evidence/REA:** Exact `Local` source comparison of failed-Start rollback versus the normal Stop path and public method contracts is **STATIC_SOURCE** only. REA executable/MCP, Cargo compilation/tests, actual concurrency/race reproduction, Windows/CPAL, GPU, Meet receiver, and long-session acceptance are **NOT EXECUTED**. The patch is a source candidate, not a proven runtime cleanup improvement.

**Next:** Continue source-only audit of one distinct high-impact Meeting failure boundary. Change code only on an independently demonstrated defect; hold speculative streaming and model optimizations until the single integrated local acceptance phase.

## Active continuation — Batch C.3 bounded estimated ASR timing validation (2026-10-13)

**Source candidate:** Extended only the existing Rust `meeting_session/source_observations.rs`, a `#[cfg(test)]` module, with a pure validator for C.2 opt-in word samples. It checks the exact active session/generation/lane/speech/audio epoch, latest revision, observed ASR window and frame count, bounded ordered word estimates, whitespace-normalized ASR transcript, and existing producer-supplied capture clock. Results are estimated capture-coordinate ranges only, **never `SourceSpanId` or a speech/translation commit permission**. No new worker, API task, model, pipeline, UI, dependency or production behavior. Two regression tests cover matching mapping and stale/misbound/malformed evidence.

**REA-style source verification:** Reviewed exact `Local` code and existing Faster Whisper word-timing contract. This is static source consistency, not actual REA CLI/MCP execution. Rust tests, actual model/word timestamp accuracy, Windows/GPU/audio, latency, and human semantic review are **NOT EXECUTED**. Matching text and sample ranges alone cannot prove a word truly occurred at that audio time.

**Anti-overdevelopment STOP:** Do not implement speculative partial clause commit, streamed TTS or further adapters before a matching compiler test and real Faster Whisper output on controlled WAV. Keep the production finalized-utterance path unchanged.

**Single next step:** Use the coordinated local acceptance phase to run the pinned Rust/Python suites and one model-backed word-timing fixture; accept further partial-clause development only if the measured results warrant it.

## Active continuation — Batch C.2 opt-in ASR alignment observations (2026-10-13)

**Source candidate:** The existing Faster Whisper worker exposes an explicit `alignment_probe=true` on its existing `transcribe` task. This is a **developer observation path only**, requiring VAD disabled plus an exact, fully readable PCM16 mono 16 kHz WAV of at most 60 seconds. It returns bounded model-estimated word times in 16 kHz sample coordinates only when every range is finite, positive, ordered and inside the observed WAV and reconstructed words match the transcript. A malformed, overlapping, missing, truncated or mismatched observation fails closed. Normal Meeting requests do not enable the flag; the canonical finalized ASR → MiLMMT → GPT-SoVITS production path is unchanged. Estimates are NOT verified phoneme, word, clause, semantic or native-capture boundaries.

**REA source/evidence boundary:** Upstream Faster Whisper `transcribe.py` on `SYSTRAN/faster-whisper` at `a52b026ac844c02117865e61968547639b60a6f8` documents `word_timestamps` and timestamp restoration. This is STATIC_SOURCE reference only; no REA CLI/MCP target, installed model, GPU, binary, runtime or competitor capture was inspected. Authored pytest cases exercise default-path nonactivation, valid shapes, invalid opt-in, WAV integrity, time bounds, overlaps and text divergence; they are **NOT EXECUTED**. Nothing in this batch proves word timing accuracy, delivered speech, latency or end-to-end streaming.

**Remaining evidence:** ASR model-estimated times must be checked against known source/audio epoch and stable transcript revision using the existing Rust `source_observations.rs` owner, with explicit unverified/uncertain timing semantics. No progressive translation/voice commit is authorized by the worker response.

**Single next step:** In test-only Rust source observations, consume the new opt-in word ranges as unverified evidence with strict window/revision/epoch binding and reject inconsistent spans. Keep `interpreter_policy.rs` nonfinal output at `WAIT` until semantic human review and target-Windows evaluation.

## Active continuation — Batch C.1 semantic source eligibility (2026-10-13)

**Source candidate:** A test-only pure `meeting_session/interpreter_policy.rs` now consumes the bounded `StableTextCandidate` result and requires a matching session/generation/lane/speech/audio-epoch scope and current revision. The existing ASR candidate now carries its scope EVEN for nonfinal stable prefixes. Advisory Indonesian tail checks identify plainly open negation/conditional/repair/connective forms, but punctuation, two ASR agreements and lag NEVER certify translation safety. All nonfinal prefixes remain `WAIT` because there is no verified word-to-audio span. A finalized whole window with matching producer-aligned span may return `COMMIT_READY` as a candidate only; this module does not mutate transcript, perform MiLMMT/TTS, or change production finalized-utterance behavior. No model or UI addition.

**Evidence:** Exact GitHub source review and five bounded Rust behavior scenarios authored. Cargo, Windows/GPU/audio, receiver, human semantic-quality and long-meeting checks remain **NOT EXECUTED**. Deterministic lexical hints cannot prove meaning, avoid hallucinations, or replace human quality review.

**Unresolved:** Audio-aligned partial clause boundaries, explicit incremental ASR worker observations, human fidelity comparisons for revisions/negation/numbers, and live eligibility/commit authority. No partial speech should be admitted without those gates.

**Single next step:** Specify and implement the minimum source-to-audio clause-boundary evidence from ASR for progressive units, not heuristic word timestamps; retain the existing finalized path until native acceptance.


## Active continuation — Batch A source and delivery authority (2026-10-12)

**Source candidate on Local:** The existing Meeting turn store now treats source insertion as a one-time pipeline admission, rejects duplicate sequences or lane/generation/utterance identities, and retains a compact retired-sequence fence when old transcripts are evicted. It checks the live canonical session before source insertion. The outbound path now requires a successful queued transition after translation lock and playback atomically claims the exact session/generation/sequence/utterance before native delivery. No new worker, engine, model, or UI; finalized-utterance production remains the only enabled path.

**Evidence ceiling:** Authored Rust regressions and exact-source review only. Cargo tests/build, Windows CPAL/VRAM, voice and meeting receiver checks are **NOT EXECUTED**. No measured latency or full Stop/capture atomicity claim.

**Unresolved:** This Batch A slice protects current finalized-turn identities, not future sample-level SourceSpanId/audio-epoch/revision ownership. Streaming alignment, revision checks and end-to-end integrated proof remain pending. PCM handoff stays test-only.

**Single next step:** Implement bounded source-span/epoch/revision observations as one cohesive source candidate; do not activate streaming until integrated acceptance. No hosted CI or incremental user-PC tests.

## Active continuation — PCM consumer ownership and shutdown proof (2026-10-12)

**Source candidate:** A PCM handoff now atomically reserves one CPU consumer before thread spawn, rejects duplicates, and releases reservation if OS thread spawn fails. A timed-out worker cannot be replaced by a second consumer; Stop also erases the buffer even when its mutex was poisoned. This change stays test-only and reuses canonical VAD and the same Meeting design. No new engine, model, UI, runtime dependency, or production CPAL routing.

**Authored regressions (not executed):** consumer claim/reclaim after failed spawn, duplicate start while a worker is deliberately blocked, start denial after Stop timeout, and poisoned-mutex audio erasure. **Exact Local source inspection only. Cargo compile/tests, target Windows GPU/audio, VB-CABLE and meeting receiver checks remain NOT EXECUTED.**

**Single next step:** compile/run targeted Rust PCM/VAD tests on one exact-SHA Local checkout in the integrated acceptance phase, then resolve observed failures before any reversible CPAL production cutover. Do not dispatch hosted CI or request incremental user-PC tests.

## Meeting V2 Batch 2A.2 — VAD callback contention observation (2026-10-12)

**Change:** the existing producer records callback lock-admission count and maximum wait in nanoseconds for You/incoming separately. Explicit support export includes four numeric aggregates only. It does not capture raw audio or transcript data and does not change the scheduler, output delivery, VAD thresholds or wait behavior. **It is not a fully nonblocking callback solution.** Observation overhead itself requires target-PC validation.

**Evidence ceiling:** source candidates and assertions only, no executable compilation/Windows model or browser proof.

## Meeting V2 Batch 2A — safer hot path without streaming takeover (2026-10-12)

**Change:** existing VAD pre-roll ring evicts BEFORE pushing, so it can remain within its preallocated capacity even when callback blocks exceed the pre-roll length. Incoming Windows loopback F32 stereo downmix reuses callback-owned `Vec<f32>` instead of allocating a new mono vector each callback. Suppression no longer takes a VAD mutex for *every suppressed frame*: the canonical playback suppression owner already resets the incoming boundary both when starting and ending self-output suppression. Existing user-visible behavior, VAD thresholds, output routing, scheduler and TTS remain unchanged.

**Evidence ceiling:** Rust deterministic unit tests added for pre-roll capacity, last-tail retention, sanitized samples and stereo downmix scratch reuse; source read only, not cargo-built on Windows/GPU. This is an incremental hot-path improvement, **not** a fully nonblocking callback: active producer and diagnostic buffers still use locks, and other incoming sample formats use the existing conversion path. Do not claim a latency improvement yet.

**Next Batch 2B:** isolate VAD and callback-expensive resampling behind a bounded, preallocated PCM handoff with explicit overflow and session-generation cancellation; measure results with a deterministic replay harness before routing production capture through it. Persistent playback must remain a separate A/B candidate; do not conflate it with capture stability.

## Meeting V2 Batch 1B — bounded full-session manual export (2026-10-12)

**Source candidate:** one existing canonical in-memory turn store retains up to 4,096 phrases and 8 MiB of text (whichever is reached first). Live and last-session review still expose at most 240 recent phrases to keep webview work bounded. Manual MD/TXT exports use the entire retained session without silently treating hidden UI phrases as lost. Exports continue to be explicit, session-scoped and never persisted automatically.

**Hard retention truth:** if actual session bounds are exceeded, oldest turns are evicted and counted as unrecoverable; export reports truncation. This is a bounded 32 GB RAM design, **not** guaranteed unlimited recordings. Continuous streaming audio and long-unbroken speech segmentation remain separate work items.

**Verification ceiling:** Rust retention tests and frontend contract added; build/runtime tests not executed yet.

## Meeting V2 Batch 1A — Source-first transcript and deferred cleanup (2026-10-12)

**Candidate:** canonical Meeting turn store now represents pending/translated/failed translation separately from playback. Incoming and outgoing recognized source is stored before model translation. Invalid output retains original source and displays "Translation unavailable". Stop invalidates pending work; all async completion checks session, generation and utterance identity.

**Incoming deferral:** queue expiry/overflow/rejected jobs are cleaned outside queue mutex; an already-recognized transcript is marked failed instead of left forever pending. Existing bounded age, admission limit and outbound precedence stay intact.

**Proof ceiling:** source code plus Rust and TS tests added, but neither compiler nor local model/RTX 3070/browser have been run. The separate Batch 1B change above now keeps a larger bounded full session for manual export while limiting the UI to 240 recent turns. No new model, streaming worker, scheduler, GPU setting or UI mode added.

## Meeting V2 — Source-first implementation continuation (2026-10-12)

**New plan:** [Meeting V2 Implementation Plan](./meeting-v2-implementation-plan.md). The user's PC is not currently available; build source and deterministic regression safety before any hardware-specific promotion. Preserve one Meeting Start/Stop flow and one RTX 3070 GPU scheduler. Next: Batch 0 (remove inert incoming style, audit capture callback blocking) then Batch 1 (reliable capture and source-first transcript state). No latency/VRAM improvement is claimed before Windows execution.

## Meeting Performance Foundation — hardware-specific instrumentation (2026-10-12)

**Scope:** adds source-only, numeric, redacted measurement for VAD-estimated speech onset/end to first native audio, helper scheduler wait by ASR/translation/TTS, and CPAL stream-build/start overhead. The production models, GPU concurrency, audio routing, VAD thresholds, and final-utterance flow remain unchanged. Device: RTX 3070 8 GB + i7-14700-class CPU + 32 GB RAM, without a usable iGPU.

**Do not overclaim:** the existing `outbound_latency_ms` is post-finalization; onset/end values are approximate, and the first native callback is not proof the Google Meet receiver heard translated speech. Real VRAM/CPU/Chrome GPU load require target Windows instruments. No native or build result yet on this revision.

**Admission sequence:** matching source/typecheck/Rust/Python tests → one pinned Windows real-Meet baseline with 10–20 phrases → cold/warm, P50/P95, Chrome active + nvidia-smi/Task Manager VRAM peaks → at least five Start/Stop cycles and 30-minute stability → evaluate one change at a time against baseline; do not start speculative streaming.

## Meeting transcript refinement — 2026-10-12

**Decision:** use WhatsApp-like left/right transcript presentation, `You` for microphone turns on the right and `Person 1` on the left for the **single combined incoming lane**. This is **not** speaker diarization and must not infer identities or separate different participants without actual speaker evidence. Display local recorded time and a readable session date/time; retain original and translation as separate fields. One reusable bubble component renders Live and Last Meeting.

**Manual export policy:** every Meeting already has a distinct runtime session ID, but only the active plus most recently ended transcript is available in memory. The new manual export uses a unique folder under `SavedProject/Transcripts/Meeting_<session-id>/`. No background transcript saving and no persistent archive listing are added. Users must export before starting another meeting or exiting. The 240-entry display window remains, while the newer bounded full-session memory policy above governs manual export. Disk spooling and multi-speaker diarization remain separate future work.

**Browser limitations:** incoming WASAPI endpoint-loopback may include non-meeting system sounds and may miss browser audio routed to another speaker. An active capture stream is not transcription or participant-identity proof. Windows two-device meeting test, simultaneous speech, suppression, 20-second defer/drop behavior and transcript export are still NOT EXECUTED for this source revision.

**Quality ceiling:** source changes and contract assertions alone. Do not call it target Windows tested, fully transcribed or diarization-ready.

