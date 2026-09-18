# Current Validation

This file owns **proof interpretation**, not a per-run diary. `next-action.md` owns continuation; GitHub remains authoritative for exact run/job metadata.

## Source Authority

Repository: `MIVUBI-STD/M-TranslateIT`

`Local` is the sole active branch for development, governance, CI, proof, continuation, and release-source validation.

One SHA never proves another SHA. A later documentation-only commit may record proof without changing the validated runtime/source identity.

## Current Source Proof

Source identity:

```text
a8ce3e11040b5511a9e0eed012e04bb04c72cc4a
fix(voice): invalidate failed synthesis runtime
```

Matching proof:

```text
Code Health                 35332249945  PASS
MiLMMT Repository Contract  35332249965  PASS
```

The matching Code Health run establishes the selected frontend, Rust, and Python source-health surfaces for that exact source identity.

Latest later source candidate:

```text
60c1cb334c611950beace20185bbfc2b78e04eca
fix(voice): remove unowned review drafts
```

This later candidate is a bounded My Voice review-file cleanup. Until its matching Code Health completes, treat it as **SOURCE PRESENT / EXACT-HEAD EXECUTED PROOF PENDING**, not PASS.

## Current Source Claims

The modernized source now establishes, subject to exact-head proof for the SHA being discussed:

- one canonical Meeting lifecycle authority with generation-bound Start/Stop and fail-closed cleanup;
- prepared Meeting output state is cleared when authority acquisition fails before Starting can be established;
- Meeting status, preflight, session state, consumers, suppression, optional incoming activation, outbound/incoming pipelines, committed turns, and Start/Stop lifecycle have explicit owners rather than one oversized command module;
- helper scheduling, request policy, transport/recovery, functional readiness, functional probe, and process/runtime I/O have explicit owners;
- live helper transport recovery remains bounded: ASR/translation may retry once only after the canonical worker restarts **and** the exact Meeting generation re-proves the required outbound AI/voice path; synthesis is not automatically replayed;
- hard My Voice synthesis failure clears the cached actor runtime so later inference reloads from canonical actor state instead of reusing a runtime that has just proved unhealthy;
- My Voice build process ownership is fail-closed: a spawned child that cannot be recorded in process state is terminated rather than left untracked;
- My Voice recording accept/retry flows preserve or restore the previous accepted take transactionally and do not claim discard when review audio could not be removed;
- storage/promotion validates actor package identity, canonical WAV shape, held-out evaluation, staging, rollback, and interrupted promotion recovery;
- CPU fallback remains explicit degraded operation for known capability absence; broad cloud/parallel fallback is absent;
- MiLMMT remains the single canonical bidirectional translation model and keeps the established deterministic/context contracts;
- existing latency hardening remains intact: bounded capture storage, efficient finalized WAV preparation, deterministic MiLMMT generation/KV cache, warm actor reuse, and V2ProPlus reference-speaker embedding caching.

## Verification surfaces

```text
Repository Verify
→ governance / Local-only routing / skills / repository contracts

Code Health
→ frontend: typecheck, build, runtime tests, source-size, bridge contract,
  reachability, virtual-route contract, npm audit
→ Rust: compiler/dead-code, Clippy, unit tests on Linux/hosted Windows when selected
→ Python: compile, Ruff/static/format, pytest on Linux/hosted Windows when selected

MiLMMT Repository Contract
→ canonical translation provider/repository contract

WorkerRuntime Lock Consistency
→ Python dependency-lock integrity

R3 Release Contract
→ controlled release-source / Windows payload proof when payload inputs change
```

Path-targeted skipped jobs are not evidence for unrelated domains.

## Proof Boundaries

### REMOTE_GITHUB

Can prove source ownership, contracts, hosted execution, bounded recovery behavior, and matching CI tests.

It does **not** prove actual Windows microphone/device behavior, real CUDA throughput, installed-package behavior, meeting-app reception, audio fidelity, or practical latency.

### LOCAL_CODE

Can additionally prove the exact checkout/toolchain/filesystem/build that actually ran locally. It still does not automatically prove real target-device acceptance.

## Target Windows

### TARGET_WINDOWS / NATIVE_ACCEPTANCE

Required for:

- physical microphone and VAD behavior;
- VB-CABLE / TranslateIT Meeting Microphone routing;
- actual meeting-application reception;
- real GPU/CPU/RAM/VRAM behavior;
- Start → Live timing;
- end-of-speech → first translated playback timing;
- audible My Voice quality and practical stability.

## Evidence Rule

```text
claim
→ semantic owner
→ matching verifier/scenario
→ exact source identity
→ completed matching evidence
→ only then PASS
```
