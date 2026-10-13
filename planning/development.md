# Active development — Meeting V2 Phase 0: baseline admission (2026-10-13)

**State:** SOURCE_CANDIDATE; speculative streaming remains frozen. Session-scoped optional incoming cleanup is a new unexecuted source candidate. This document owns only the active continuation and one next step. Historical 2026-10-12/13 batch explanations remain in Git commits, not as concurrent “Active continuation” instructions.

**Authority:** `MIVUBI-STD/M-TranslateIT`, branch `Local`. Source review baseline before this consolidation: `f7ed311e94bc9f87535b04a27f5dc797d8c2ec26`. Re-pin the exact `Local` HEAD for any later execution; no ancestor test result certifies a modified SHA.

**Goal:** Establish a falsifiable, source-consistent baseline for safe Meeting V2 work before an audio-pipeline production cutover. Preserve the current Windows local-first Indonesian ↔ English product, one Tauri/Rust Meeting owner, one Python AI worker and one shared GPU scheduler. Do not add a parallel engine, new AI provider, hosted CI, cloud fallback, UI streaming controls, or duplicate test infrastructure.

## Current source truth

- **Production Meeting remains finalized-utterance only:** physical microphone → VAD/finalized source → Faster Whisper → MiLMMT → GPT-SoVITS complete WAV → native output/virtual microphone. Incoming English→Indonesian remains optional text; outbound takes priority.
- **Source hardening candidates already landed:** Meeting Start/Stop ownership and cleanup, session-scoped deferred incoming and producer cleanup, stale-job rejection, cancellation checks, source-first transcripts and explicit bounded manual export. Detached optional Meeting Sound activation now requests session-scoped Stop; capture shutdown holds its ownership lock through thread join and VAD teardown, with a local owner-decision regression authored. These code paths have not received matching executable regression acceptance after the source changes.
- **Experimental only:** bounded PCM handoff/replay, ASR source-observation and semantic eligibility are `#[cfg(test)]` Rust modules; alignment-probe input is opt-in. No partial transcript may authorize progressive translation or speech playback yet.
- **A VAD metadata flag is not runtime proof:** `partial_asr_enabled` is declared in a profile but does not activate a production partial-ASR inference path.
- **Source-observed limitations:** uninterrupted utterances exceeding 60 seconds are discarded by the current finalized buffer; production callback synchronization/allocation needs measured safety; the single nonpreemptive helper scheduler exposes waits but cannot guarantee low conversation latency. Do not present these as resolved by unexecuted prototypes.
- **Benchmark reference:** documented Gemini 3.5 Live Translation streaming behavior is a comparison target, not evidence of its proprietary model internals or of TranslateIT equivalence. Use existing [REA investigation guidance](../docs/knowledge/operations/rea-investigation.md) only when an authorized, specific external evidence question requires it; the REA CLI/MCP has not been executed for this baseline.

## Proof and acceptance boundary

- Latest recorded integrated developer execution is on `230ca94faae178659a7738fa82399485c267c991`: existing frontend/Rust/Python source tests and limited worker/UI smoke completed as described in [Current Validation](../docs/knowledge/current-validation.md). It does **not** certify the 44 subsequent source commits up to the pre-consolidation baseline.
- Current proof ceiling: **STATIC_SOURCE** for these later revisions, including optional capture shutdown. Matching Cargo/frontend/worker tests, real ASR timing, microphone/virtual-mic routing, browser receiver, latency P50/P95, VRAM, 30-minute endurance, installed release and REA executable are **NOT EXECUTED**.
- The existing [Meeting V2 implementation plan](./meeting-v2-implementation-plan.md) owns future technical batches and risk-specific regressions. The [target-Windows runbook](../docs/knowledge/operations/target-windows-performance.md) owns one coordinated executable acceptance; do not multiply checklists here.
- Promotion requires a clean exact-SHA build, relevant existing tests, controlled long-speech/Stop/queue/audio-failure scenarios and real receiver/semantic-quality measurements. Keep finalized output as the control until a candidate meets the existing release/quality gates.

## Single next step

**Admit one pinned `Local` source candidate to the existing integrated local build-and-Meeting baseline gate** (frontend, Rust and worker checks first; then target-Windows/RTX 3070/audio receiver and latency if the source gate passes). Treat any unavailable execution as `NOT EXECUTED`; do not request incremental per-commit tests or dispatch hosted workflows. Use the results to select the first independently evidenced Phase 1 correction, prioritizing long-utterance loss and audio callback stability.

**STOP rule:** no production streaming, alternate scheduler, new provider, speculative TTS fragment or additional REA wrapper before matching evidence separates the bottleneck and verifies safety.
