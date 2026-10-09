---
id: document.foundation.realtime-latency-architecture
class: DOCUMENT
domain: foundation
role: ARCHITECTURE
authority: CANONICAL
lifecycle: ACTIVE
---

# TranslateIT — Realtime Latency Architecture

**Status:** Approved target architecture; implementation remains evidence-gated  
**Updated:** 2026-09-09  
**Scope:** required outbound Meeting latency without reducing translation, ASR, voice, ordering, or safety quality

## Goal

Reduce perceived end-of-speech → first translated playback latency and prevent latency from accumulating during normal consecutive speech, while preserving the current canonical models and Meeting reliability contracts.

This file defines the target architecture. It does not claim the phases below are already implemented or TARGET_WINDOWS-proven.

## Non-negotiable invariants

- One Tauri/Rust Meeting lifecycle owner and one canonical Python worker remain authoritative.
- Finalized utterances remain the only normal outbound AI input.
- ASR model/beam/language/VAD quality, MiLMMT model/prompt/context/generation policy, GPT-SoVITS V2ProPlus actor/seed/quality mode, and selected voice must not be weakened for latency.
- English TTS playback remains serialized, ordered, bounded, and at-most-once.
- Capture may continue while previous output is processing or playing.
- No unbounded queue, parallel TTS engines, second worker, speculative cloud fallback, or user-facing Realtime/Quality mode.
- Stale generation/session/utterance work never becomes current output.
- Stop revokes output authority before cleanup; queued old audio is never replayed after Stop/restart/wake.
- Failure is explicit. Backpressure or output failure must not silently drop, reorder, duplicate, or replay speech.

## Stable target flow

```text
physical microphone
→ finalized utterance
→ bounded finalized queue
→ ASR
→ ID→EN translation
→ selected voice TTS preparation
→ bounded playback handoff
→ one serialized Rust playback owner
→ TranslateIT Meeting Microphone
```

The key separation is **AI preparation vs playback ownership**. Python inference remains serialized by the canonical helper scheduler; Rust playback becomes independently serialized so the next finalized utterance can use the Python worker while the previous English audio is already playing.

## Phase A — bounded playback decoupling

This is the preferred first architecture change when continuous-speech `queue_ms` grows because the outbound consumer waits for previous playback to finish.

Keep the current full-WAV TTS and current `deliver_meeting_output_wav` algorithm unchanged. Add one dedicated Meeting playback consumer and a `sync_channel(1)`-equivalent handoff.

Bound:

```text
currently playing = max 1
pending playback  = max 1
```

The AI/outbound consumer may therefore prepare at most **one utterance ahead**. It must not generate an unbounded audio backlog.

A playback job carries generation, session, event sequence, utterance identity, source WAV path, actor/output identity, and timing context. Playback accepts jobs only for the authoritative generation and in event-sequence order.

If the one pending slot is occupied, AI preparation experiences bounded backpressure until the playback consumer advances. If the generation becomes stale or the bounded wait cannot complete safely, the turn becomes an explicit interrupted/attention state; it is never silently emitted late.

Ownership changes required by this separation:

- the playback consumer owns `deliver_meeting_output_wav` execution;
- self-output suppression covers **actual playback**, not time spent waiting in the queue;
- first-playback and delivery completion timing are reported by the playback owner;
- the playback owner deletes TTS temp WAVs after success/failure/cancellation;
- Stop cancels current playback, closes the producer, rejects pending jobs, cleans their temp files, and joins the playback consumer.

No audio synthesis or resampling math changes in Phase A.

## Phase B — persistent Meeting output stream

Introduce only if TARGET_WINDOWS shows repeated native stream/device setup remains a meaningful delivery cost or source of jitter after Phase A.

The Rust Meeting output owner then opens the selected endpoint once during `Starting`, proves a real silent callback, and retains that exact stream through the authoritative Meeting generation.

```text
Starting
→ resolve exact route
→ validate output config
→ build/start CPAL stream
→ observe silent callback
→ keep stream alive
→ commit Live
```

The same stream that proves native callback readiness becomes the playback stream; do not open a second probe-only stream and then reopen per utterance.

When idle, the callback emits silence. Callback work is restricted to bounded sample copy, generation/cancel atomics, first-playback signaling, and non-blocking buffer-boundary handoff. No file I/O, device enumeration, Python/JSON work, allocation-heavy conversion, or ordinary logging belongs in the callback.

A native callback error marks the output runtime failed for that generation. Do not automatically rebuild/replay uncertain audio mid-session. Subsequent delivery fails closed until explicit recovery through the normal lifecycle.

## Phase C — strong functional-proof rebinding

Introduce only if warm Start → Live is materially dominated by repeating the full ASR→translation→voice→ASR functional fixture on an otherwise unchanged healthy worker.

A reusable AI proof must be bound to a strong identity, not elapsed time or file presence. Minimum identity:

```text
helper worker generation token
+ readiness/invalidation epoch
+ ASR model id + executed device/compute type
+ MiLMMT model id + pinned revision + executed device/precision
+ selected voice engine/revision
+ exact actor token/fingerprint
```

Full functional proof remains required when no matching proof exists. A later Meeting generation may rebind the proof only after a cheap worker status confirms the exact same identity and no invalidation event occurred.

Invalidate the proof on at least:

- helper exit/restart/transport replacement;
- hard required outbound ASR/translation/TTS failure;
- actor selection/promotion/rebuild/change;
- model/runtime device or precision identity change;
- explicit runtime repair/restart;
- power lifecycle invalidation where runtime continuity is uncertain.

Normal Stop does not need to invalidate a healthy proof by itself; a Stop that hard-cancels/restarts the worker already changes worker generation.

Microphone and native output readiness are still proven for **every new Meeting Start**. AI proof rebinding must never turn PR-053 into file-presence readiness.

## Phase D — quality-preserving TTS fragments

This is last, not first. Enable only when TARGET_WINDOWS shows `tts_ms` remains the material first-audio bottleneck after safer overlap/output work.

The pinned GPT-SoVITS runtime exposes `return_fragment=True` while keeping `streaming_mode=False`. That path may be evaluated as an internal first-audio transport optimization, but it is not assumed equivalent until target quality evidence exists.

Rules:

- never enable `streaming_mode=True` merely for speed;
- never enable fixed-length/lower-quality chunk options;
- fragments from different utterances never interleave;
- event/generation/actor identity travels with every fragment;
- once any fragment has been played, failure must not replay that utterance from the beginning;
- full-WAV remains the validated baseline until fragment quality and stability pass TARGET_WINDOWS comparison;
- no user-facing quality/realtime toggle is introduced.

Acceptance for fragment promotion requires the same selected voice, seed, model and reference plus target listening/speaker-fidelity evidence showing no material quality regression.

## Metrics

Retain current stage metrics and distinguish preparation from playback pressure:

```text
speech_boundary_ms
finalization_ms
queue_ms                finalized utterance → AI owner
asr_ms
translation_ms
tts_ms
playback_queue_ms       TTS ready → first playback ownership
output_prepare_ms       Rust decode/resample/preparation when applicable
delivery_ms             playback execution → first audible frame
outbound_latency_ms     finalized utterance → first translated playback
```

Also expose bounded counters/state for playback queue depth, playback backpressure, interrupted queued output, callback errors, and persistent-stream failures when those phases exist. Diagnostics remain redacted and must not log conversation text/audio.

## Rollout order

Do not implement all phases together.

```text
TARGET_WINDOWS baseline
→ measured continuous-speech queue growth?  → Phase A
→ native delivery/open jitter still material? → Phase B
→ warm Start dominated by full AI proof?     → Phase C
→ TTS still dominates first-audio latency?   → Phase D
```

Each phase must preserve the previous accepted behavior and have its own rollback boundary. A later optimization is not justification to weaken an earlier safety invariant.

## Source acceptance

REMOTE_GITHUB/source proof may establish bounded queue logic, order, generation rejection, cleanup ownership, compile/lint/unit contracts, identity matching, and failure-state behavior.

TARGET_WINDOWS remains required for actual microphone/device behavior, playback callback stability, meeting-app reception, real Start → Live, end-to-end latency, resource pressure, speaker fidelity, and repeated-session claims.

The architecture is successful only when measured latency improves **without** quality reduction, stale output, duplicate playback, unbounded backlog, or weaker readiness truth.
