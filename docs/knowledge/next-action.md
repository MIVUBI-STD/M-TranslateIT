# Next Action

## Current Status

- `Local` is the sole development, CI, proof, continuation, and release-source authority.
- Meeting/Text/My Voice/runtime architecture is structurally hardened.
- Translation quality uses a 180-case regression corpus plus separate 40-case held-out benchmark; ASR has 60 acceptance cases; TTS/My Voice has 30 synthesis cases.
- Quality Readiness is fail-closed across Translation + ASR + TTS evidence.
- Diagnostics expose queue/drop, stage timing, runtime reuse, asset checks, context count, and terminology count.
- Zero-waste hardening avoids repeated warm asset scans and caches runtime capability state.
- App update is signed, one-shot at startup, activity-safe, and non-polling. Current auto-update is app-only: NSIS `/UPDATE` preserves the external runtime only after compatibility verification. Payload/model/runtime identity changes require full Setup + Payload.

## Active Boundary

Continue REMOTE_GITHUB work only for falsifiable quality, reliability, accessibility, release, or contract defects.

Do not add extra providers, languages, cloud fallback, document/image translation, speculative runtime abstractions, or unmeasured performance optimizations.

Physical microphone behavior, CUDA/resource pressure, meeting-app reception, audible voice quality, practical latency, long-session stability, clean-machine install, and updater end-to-end behavior require **TARGET_WINDOWS / NATIVE_ACCEPTANCE** evidence.

## Next Step

When native testing is available:

1. Run `docs/knowledge/operations/target-windows-performance.md` with outbound-only baseline first.
2. Record speech boundary, finalization, queues, ASR, translation stages, TTS, delivery, total latency, drops, CPU/RAM/GPU/VRAM, and meeting-app reception.
3. Confirm warm runtime reuse and stable capability-probe count.
4. Fix only the first measured bottleneck.
5. Validate Text, My Voice, incoming Meeting, device recovery, long-session stability, clean-machine install, then signed updater end-to-end.
6. For updater release procedure use `docs/knowledge/operations/signed-updater-release.md`.

Until then preserve one canonical MiLMMT pipeline, bounded queues, at-most-once output, terminology determinism, generation authority, fail-closed readiness, and the app-only updater compatibility boundary.
