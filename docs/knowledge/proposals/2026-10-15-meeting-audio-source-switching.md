# Meeting Audio Source Switching — implementation contract (2026-10-15)

Status: SOURCE INTEGRATED / COMPILATION AND TARGET_WINDOWS NOT EXECUTED.
Branch authority: Local only. This document is an implementation handoff, not a runtime proof.

## User-visible behavior

One selected physical microphone, one virtual Meeting microphone, one two-way source selector:
- **Original Microphone**: physical microphone PCM is routed directly to the existing virtual output, bypassing ASR, translation and TTS.
- **TranslateIT Audio**: existing finalized speech → ASR → translation → TTS → the same virtual output.
Only one source may reach the virtual output at a time. The calling application's microphone selection does not change when switching. No audio mixer, second virtual driver, Python audio router, or automatic fallback to raw speech.

## Existing canonical owners

- `engine/audio/live_capture.rs`: one CPAL physical microphone capture owner; extend its existing converted mono PCM callback handoff without opening another microphone stream.
- `engine/audio/meeting_output_runtime.rs`: one CPAL virtual output owner. Current implementation creates a new stream per WAV; switching requires a session-owned output stream and a bounded PCM handoff, not a second output owner.
- `commands/meeting_session/playback_runtime.rs`: existing bounded TTS playback queue, cancellation and ordering; preserve generation and stale-output protection.
- `commands/meeting_session/lifecycle.rs` and `preflight.rs`: authoritative session start/stop and readiness; Original requires functional microphone and virtual route but not an AI worker. TranslateIT still requires existing AI/voice checks.
- `commands/virtual_mic_route.rs`: the supported standard VB-CABLE pair remains authoritative. Do not silently select other providers.
- `src/pages/Meeting.svelte` and `src/app/bridge/runtimeApi.ts`: expose one source selector and its native acknowledgment.

## Implementation invariants

1. Store an enum `original | translateit` as the active outbound source, with one session-bound source epoch incremented on each accepted switch.
2. Native command verifies active Meeting authority, updates the source epoch and acknowledges the new mode; UI must not claim a successful switch before acknowledgment.
3. Switching to Original cancels queued/in-flight translated playback; old TTS results carry the prior epoch and are discarded even if worker completion arrives later. Switching back to TranslateIT resets the raw buffer and finalized utterance boundary, and requires translation readiness before activation.
4. Never emit original PCM during TranslateIT mode or TTS PCM during Original mode. If source data is absent, render silence; do not substitute another device or source.
5. Original audio uses a bounded nonblocking PCM ring buffer; avoid allocation, filesystem I/O, waiting, heavy resampling or blocking mutexes inside CPAL callbacks. Handle native input/output sample rate and channel mismatches explicitly, with bounded drift/overflow behavior.
6. Existing Meeting Stop revokes session authority, clears both buffers, cancels pending TTS and releases audio handles. A subsequent session cannot consume stale frames.
7. Incoming Meeting Sound is optional and independent of the outbound source selector; keep its safety constraints.
8. Do not change VB-CABLE driver names or rewrite endpoint registry properties as part of this feature. Windows naming is a separate gated task; preserve existing route detection until endpoint identity migration is proven.
9. Avoid a second full capture pipeline or separate UI settings mode. The normal user sees exactly Original Microphone and TranslateIT Audio.

## Acceptance matrix

- Original voice passes through the existing virtual microphone with AI worker unavailable; actual receiver confirms speech.
- TranslateIT sends only translated TTS; raw voice never leaks to virtual output.
- Repeated switching during speech, queued TTS and Stop produces no delayed or duplicate speech.
- Input and output with differing sample rates/channel counts remain intelligible and bounded.
- A missing microphone or VB-CABLE pair fails explicitly without silent fallback.
- Restart, five start/stop cycles and a sustained meeting have bounded buffer/memory and no orphan streams.
- Rust unit tests cover epoch, mode transition, queue discard, silence/underflow, bounded buffer and stop.
- Target Windows validates real microphone, VB-CABLE and a separate meeting receiver; source tests alone cannot establish delivery.

## External reference

RustAudio CPAL feedback example: https://github.com/RustAudio/cpal/blob/master/examples/feedback.rs
CPAL callback guidance: https://docs.rs/cpal/latest/cpal/

The feedback example assumes compatible f32 stream configurations; TranslateIT must not copy that assumption blindly.

## Source work completed on Local (2026-10-15)

- `a230b516`: existing CPAL microphone capture now offers a gated mono PCM tap; a bounded raw audio queue handles underflow, format changes and channel/sample conversion. Authored five focused Rust unit tests; unexecuted.
- `0faf8cf0`: existing native Meeting output owner exposes an Original-voice stream through the same already-prepared VB-CABLE playback endpoint. This is internal only and NOT YET CONNECTED to Meeting Start, Stop, mode authorization or UI.
- `1ad8c80d`: source hardening for native output startup timeout and bounded cleanup ownership.
- These are **partial source components**, not a working user-visible source switch. Do not claim Original audio reaches meeting applications or show a live selector before the missing integration lands.

## Immediate integration still required

1. Bind Original/TranslateIT mode and incrementing source epoch to authoritative Meeting session start/switch/stop. Start/Stop must clean the native Original output owner and PCM buffers.
2. Gate stale ASR/translation/TTS work and the existing TTS playback callback against that epoch. An old WAV must remain silent even when the mode cycles TranslateIT → Original → TranslateIT.
3. Add a native Tauri source-selection command, status projection and a two-option control in Meeting. UI success must require native acknowledgment.
4. Ensure original capture bypasses the AI functional-start requirement where applicable; keep translation readiness checks only for TranslateIT mode.
5. Source review/compile tests and then eventual integrated TARGET_WINDOWS audio-device and meeting receiver acceptance. Do not request per-commit PC tests.

## Source integration update — 2026-10-15

- `2b595985`: Meeting now has a native two-choice source command, Rust/Svelte bridge and selector while an existing translation session is Live. Source epoch gates ASR→translation→TTS work, queued playback and native callback. Switching resets the outbound VAD boundary; Meeting Stop attempts both output cleanup paths.
- `0611c745`: fixed the `source_epoch` field placement to the native playback cursor; this was an identified source error caught during static inspection.
- Native Original output subsequently gained a bounded first-output-callback check and a unit test for a TranslateIT → Original → TranslateIT epoch round trip. Authored tests remain NOT EXECUTED.
- **Current scope limitation:** Start is still the existing AI-dependent `Start Translation` path. Users can switch Original/TranslateIT only after Meeting becomes Live. Original-only startup without AI readiness, Windows device renaming and custom branded driver are separate changes and are NOT claimed.
- Production-level validation remains UNKNOWN: no compiler, CPAL callback execution, real VB-CABLE receiver, latency or multi-device tests have been run for this revision. One selected virtual device is retained; no Python output pipeline or second physical microphone capture was introduced.

## Next code delivery

The source selector, epoch-gated TTS, and Original PCM output now exist on Local. Next, address any source-level compile or contract findings, then perform the one authorized integrated local build/package and TARGET_WINDOWS microphone-receiver acceptance. Original-only startup without AI readiness and device naming in Windows remain separate, unapplied product scope. Do not claim audio reception or validation before executed evidence.
