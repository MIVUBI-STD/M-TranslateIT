## Meeting V2 Batch 1A — Source-first transcript and deferred cleanup (2026-10-12)

**Candidate:** canonical Meeting turn store now represents pending/translated/failed translation separately from playback. Incoming and outgoing recognized source is stored before model translation. Invalid output retains original source and displays "Translation unavailable". Stop invalidates pending work; all async completion checks session, generation and utterance identity.

**Incoming deferral:** queue expiry/overflow/rejected jobs are cleaned outside queue mutex; an already-recognized transcript is marked failed instead of left forever pending. Existing bounded age, admission limit and outbound precedence stay intact.

**Proof ceiling:** source code plus Rust and TS tests added, but neither compiler nor local model/RTX 3070/browser have been run. Current 240-turn retention cap remains and needs a separate bounded full-session export revision. No new model, streaming worker, scheduler, GPU setting or UI mode added.

## Meeting V2 — Source-first implementation continuation (2026-10-12)

**New plan:** [Meeting V2 Implementation Plan](./meeting-v2-implementation-plan.md). The user's PC is not currently available; build source and deterministic regression safety before any hardware-specific promotion. Preserve one Meeting Start/Stop flow and one RTX 3070 GPU scheduler. Next: Batch 0 (remove inert incoming style, audit capture callback blocking) then Batch 1 (reliable capture and source-first transcript state). No latency/VRAM improvement is claimed before Windows execution.

## Meeting Performance Foundation — hardware-specific instrumentation (2026-10-12)

**Scope:** adds source-only, numeric, redacted measurement for VAD-estimated speech onset/end to first native audio, helper scheduler wait by ASR/translation/TTS, and CPAL stream-build/start overhead. The production models, GPU concurrency, audio routing, VAD thresholds, and final-utterance flow remain unchanged. Device: RTX 3070 8 GB + i7-14700-class CPU + 32 GB RAM, without a usable iGPU.

**Do not overclaim:** the existing `outbound_latency_ms` is post-finalization; onset/end values are approximate, and the first native callback is not proof the Google Meet receiver heard translated speech. Real VRAM/CPU/Chrome GPU load require target Windows instruments. No native or build result yet on this revision.

**Admission sequence:** matching source/typecheck/Rust/Python tests → one pinned Windows real-Meet baseline with 10–20 phrases → cold/warm, P50/P95, Chrome active + nvidia-smi/Task Manager VRAM peaks → at least five Start/Stop cycles and 30-minute stability → evaluate one change at a time against baseline; do not start speculative streaming.

## Meeting transcript refinement — 2026-10-12

**Decision:** use WhatsApp-like left/right transcript presentation, `You` for microphone turns on the right and `Person 1` on the left for the **single combined incoming lane**. This is **not** speaker diarization and must not infer identities or separate different participants without actual speaker evidence. Display local recorded time and a readable session date/time; retain original and translation as separate fields. One reusable bubble component renders Live and Last Meeting.

**Manual export policy:** every Meeting already has a distinct runtime session ID, but only the active plus most recently ended transcript is available in memory. The new manual export uses a unique folder under `SavedProject/Transcripts/Meeting_<session-id>/`. No background transcript saving and no persistent archive listing are added. Users must export before starting another meeting or exiting. The current 240-entry bounded buffer may omit early phrases; its incomplete warning is retained. Full-session temporary spooling and multi-speaker diarization are separate future changes requiring explicit implementation and acceptance.

**Browser limitations:** incoming WASAPI endpoint-loopback may include non-meeting system sounds and may miss browser audio routed to another speaker. An active capture stream is not transcription or participant-identity proof. Windows two-device meeting test, simultaneous speech, suppression, 20-second defer/drop behavior and transcript export are still NOT EXECUTED for this source revision.

**Quality ceiling:** source changes and contract assertions alone. Do not call it target Windows tested, fully transcribed or diarization-ready.

