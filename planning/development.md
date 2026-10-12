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

