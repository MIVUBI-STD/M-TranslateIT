# Next Action

## Current Status

- `Local` remains the sole active authority for development, CI, proof, continuation, and release-source validation.
- Development foundation, capability/proof taxonomy, toolchain authority, and one Windows developer entrypoint are active.
- Structural remediation is complete across Meeting, helper bridge, My Voice, setup, native-close, diagnostics, Windows audio, and local voice-runtime ownership.
- Current source `0d1906299524b0c7ab95a80389ca95c4802ee868` passed Code Health `35369634034` and MiLMMT Repository Contract `35369634052`.
- My Voice live inference/actor validation remains in `voice_lab_gpt_sovits.py`; one-shot dataset preparation, training, candidate evaluation and package construction now live in `voice_lab_gpt_sovits_build.py`. The live provider shrank from ~35.8 KB to ~13.7 KB without changing worker protocol.
- Earlier flow hardening remains active: legacy-safe setup checkpoint persistence, single-owned virtual-route prepare/bind, centralized Starting rollback cleanup, native app-exit helper shutdown, narrow My Voice exit guards, helper Start idempotence, power-transition readiness invalidation, reusable diagnostic readiness, guided-corpus extraction, and domain-level recording/build handoff.
- Diagnostics expose outbound stage timing and queue/drop counters needed for target performance evidence.

## Active Boundary

REMOTE_GITHUB/source work is complete for the currently identified architecture and flow defects.

Do not add more source abstractions merely for file shape or speculative latency. Physical microphone/device behavior, real CUDA throughput, CPU/RAM/GPU/VRAM pressure, meeting-app reception, Start → Live timing, audible voice quality, and end-of-speech → first translated playback require **TARGET_WINDOWS / NATIVE_ACCEPTANCE**.

## Next Step

When local/native testing becomes available:

1. Run `docs/knowledge/operations/target-windows-performance.md` with outbound-only baseline first.
2. Record speech-boundary, finalization, queue, audio-prepare, ASR, translation, TTS, delivery, total latency, overflow-drop, and eviction from Diagnostics.
3. Route only the measured first bottleneck:
   - queue growth → bounded playback/AI decoupling;
   - native output jitter → persistent output stream;
   - warm Start proof cost → strong proof reuse/rebinding;
   - TTS dominance → quality-preserving fragments.

Until then, preserve model/voice quality, generation authority, at-most-once output, bounded queues, and fail-closed readiness.
