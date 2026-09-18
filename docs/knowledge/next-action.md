# Next Action

## Current Status

- `Local` remains the sole active authority for development, governance, CI, proof, continuation, and release-source validation.
- The development foundation is modernized: capability gate, proof taxonomy, development discipline, toolchain authority, repository enforcement, and one thin Windows developer entrypoint are active.
- Structural remediation is complete for the former large native-runtime debt:
  - Meeting orchestration is split into lifecycle, preflight, state, consumer, suppression, incoming-activation, pipeline, and committed-turn owners.
  - Helper bridge is split into transport, request policy, functional readiness/probe, runtime, and scheduler owners.
  - My Voice storage/promotion is split from build lifecycle.
  - Former large-file exemptions for `helper_bridge.rs`, `helper_bridge_runtime.rs`, `meeting_session.rs`, `voice_lab.rs`, and `voice_lab_build.rs` are removed. `voice_lab_recording.rs` keeps a stricter 24 KB ratchet, below the normal Rust 30 KB budget.
- Targeted reliability hardening now also covers:
  - prepared Meeting output cleanup when Start authority acquisition fails;
  - fail-closed ownership of My Voice child build processes;
  - transactional My Voice review/accept/rollback cleanup;
  - generation-bound functional re-proof after live helper transport recovery;
  - invalidation of a cached My Voice runtime after hard synthesis failure;
  - removal of unowned review WAVs if draft state cannot be retained.
- Source identity `a8ce3e11040b5511a9e0eed012e04bb04c72cc4a` passed Code Health run `35332249945` and MiLMMT Repository Contract run `35332249965`.
- Latest source candidate `60c1cb334c611950beace20185bbfc2b78e04eca` adds only the final unowned-review-draft cleanup. Do not claim its exact-head Code Health PASS until the matching run completes.

## Active Boundary

REMOTE_GITHUB can now establish the modernized source ownership, static/runtime contracts, CI source health, bounded recovery policy, and the targeted reliability fixes above.

It still cannot establish physical microphone scheduling, real CUDA throughput, CPU/RAM/GPU/VRAM pressure, Meeting Microphone reception inside the actual meeting application, Start → Live time, speaker fidelity, or real end-of-speech → first translated playback latency.

Those remain **NATIVE_ACCEPTANCE / TARGET_WINDOWS proof**.

Do not reopen structural decomposition merely because a file can be split further. New refactors require evidence of a wrong owner, duplicated truth, unsafe lifecycle, or measured bottleneck.

## Next Step

1. Require exact-head Code Health for the latest source candidate.
2. Once green, run `docs/knowledge/operations/target-windows-performance.md` on TARGET_WINDOWS using the built-in voice outbound-only baseline.
3. Measure the first real bottleneck before changing architecture:
   - continuous-speech queue growth → bounded playback/AI decoupling;
   - native output open/delivery jitter → persistent output-stream work;
   - warm Start dominated by functional proof → stronger proof reuse/rebinding;
   - TTS still dominant → quality-preserving fragment delivery only if justified.

Preserve translation/model/voice quality, generation authority, at-most-once delivery, bounded queues, and fail-closed readiness.
