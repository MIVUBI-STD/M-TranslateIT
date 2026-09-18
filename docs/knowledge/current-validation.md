# Current Validation

This file owns **proof interpretation**, not a per-run diary. `next-action.md` owns continuation; GitHub remains authoritative for exact run/job metadata.

## Source Authority

Repository: `MIVUBI-STD/M-TranslateIT`

**Local-only source authority:** `Local` is the sole active branch for development, governance, CI, proof, continuation, and release-source validation.

one SHA does not prove another SHA. A later documentation-only commit may record proof without changing the validated runtime/source identity.

## Current Source Proof

Current source identity:

```text
0d1906299524b0c7ab95a80389ca95c4802ee868
style(voice): match provider formatter spacing
```

Matching proof:

```text
Code Health                 35369634034  PASS
MiLMMT Repository Contract  35369634052  PASS
```

That exact-head Code Health run passed Linux and hosted Windows Python compile/static/format/contract tests for the changed local-worker domain. The matching MiLMMT contract also passed.

Earlier frontend/Rust architecture and route ownership remain covered by exact-head source proof `49a7db2fcb511c822b3379924397fe4fd888863d` / Code Health `35368207537` for those unchanged domains.

one SHA does not prove another SHA. Claims must use the matching proof surface for the source domain that changed.

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
- First Setup persists compact step 4 through a legacy-safe checkpoint encoding, so relaunch does not regress to the route step;
- fresh Meeting Start has one route-preparation owner in `commands/runtime.rs`; lifecycle binds that exact prepared pair to the authoritative generation before resource activation, and the validator forbids duplicate preparation;
- common Starting rollback cleanup has one lifecycle owner instead of repeated cleanup tails;
- native exit blocks active/pending My Voice and non-Meeting runtime ownership, converges Meeting Stop, and explicitly shuts down the helper worker before process exit;
- public helper Start is idempotent when the canonical worker is already ready;
- suspend/resume invalidates cached functional readiness even if the helper survives;
- diagnostic/setup readiness reuses only proof bound to the current helper generation; Meeting Start still performs generation-bound functional proof;
- My Voice build consumes narrow recording-domain facts rather than the full UI recording DTO; the guided prompt corpus has its own static owner;
- live GPT-SoVITS actor validation/runtime/synthesis is isolated from one-shot dataset preparation, bounded training, candidate evaluation, and candidate package construction; build orchestration does not sit on the worker inference hot path;
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
