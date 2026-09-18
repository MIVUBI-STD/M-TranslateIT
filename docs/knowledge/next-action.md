# Next Action

## Current Status

- `Local` is the sole active authority for development, CI, proof, continuation, and release-source validation.
- Development foundation, capability/proof taxonomy, toolchain authority, and one Windows developer entrypoint are active.
- Structural remediation is complete for Meeting, helper bridge, and My Voice ownership. Large-file exemptions for helper, Meeting, `voice_lab.rs`, and `voice_lab_build.rs` are removed; `voice_lab_recording.rs` remains under its stricter 24 KB ratchet.
- Reliability hardening covers Start rollback cleanup, tracked My Voice build processes, transactional review/accept cleanup, generation-bound helper recovery re-proof, failed TTS runtime invalidation, and orphan review-WAV cleanup.
- Source `a8ce3e11040b5511a9e0eed012e04bb04c72cc4a` passed Code Health `35332249945` and MiLMMT Repository Contract `35332249965`.
- Latest source candidate `aa25d6c515937e8bba5973a455ce7e2cdf39afd6` exposes existing outbound timing and queue/drop counters in Diagnostics. Exact-head Code Health is pending.

## Active Boundary

REMOTE_GITHUB now covers source ownership, CI contracts, bounded recovery, and source-side observability.

It still cannot prove physical microphone/device behavior, real CUDA throughput, CPU/RAM/GPU/VRAM pressure, meeting-app reception, Start → Live time, audible voice quality, or real end-of-speech → first translated playback latency. Those require **TARGET_WINDOWS / NATIVE_ACCEPTANCE**.

Do not reopen structural decomposition or implement latency architecture phases without measured evidence.

## Next Step

1. Require exact-head Code Health for the latest candidate.
2. Run `docs/knowledge/operations/target-windows-performance.md` with outbound-only baseline first.
3. Use Diagnostics for speech-boundary, finalization, queue, audio-prepare, ASR, translation, TTS, delivery, total latency, overflow-drop, and eviction evidence.
4. Route only the measured first bottleneck:
   - queue growth → bounded playback/AI decoupling;
   - native output jitter → persistent output stream;
   - warm Start proof cost → strong proof reuse/rebinding;
   - TTS dominance → quality-preserving fragments.

Preserve model/voice quality, generation authority, at-most-once output, bounded queues, and fail-closed readiness.
