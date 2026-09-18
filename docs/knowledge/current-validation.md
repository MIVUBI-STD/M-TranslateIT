# Current Validation

This file owns **proof interpretation**, not a per-run diary. `next-action.md` owns continuation; GitHub remains authoritative for exact run/job metadata.

## Source Authority

Repository: `MIVUBI-STD/M-TranslateIT`

**Local-only source authority:** `Local` is the sole active branch for development, governance, CI, proof, continuation, and release-source validation.

one SHA does not prove another SHA. A later documentation-only commit may record proof without changing the validated runtime/source identity.

## Current Source Proof

Exact proof identity is read from GitHub Actions for the exact `Local` SHA and changed domain under discussion. This file does not pin a mutable "current source SHA", because doing so becomes stale as soon as `Local` advances.

Current interpretation rules:

- use the latest completed matching verifier for the exact changed source domain;
- ancestor proof remains valid only for source domains unchanged since that ancestor;
- a documentation-only commit does not invalidate unchanged runtime proof, but it also does not create new runtime proof;
- queued, running, skipped, cancelled, or unrelated jobs are not PASS;
- one SHA does not prove another SHA where the relevant source changed.

Recent verified baseline relevant to the current work:

```text
Code Health                 a26d5e33b31eee6753ff1bc7f09b847f11770bc3  PASS
MiLMMT Repository Contract  848329bb21b45d72eccde0a19160490c9d843f22  PASS
```

Later translation-quality-only commits must use their own MiLMMT Repository Contract result before being claimed as verified. GitHub remains authoritative for exact run/job identifiers.

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
- existing latency hardening remains intact: bounded capture storage, efficient finalized WAV preparation, deterministic MiLMMT generation/KV cache, warm actor reuse, and V2ProPlus reference-speaker embedding caching;
- translation inference now exposes tokenization, model-inference, decode, and inference-throughput telemetry through Meeting Diagnostics without adding a second runtime owner;
- standalone multi-chunk translation aggregates those telemetry values across the full request rather than reporting only the final chunk;
- translation-quality evaluation supports baseline-vs-candidate critical-regression detection, grouped critical-pass-rate deltas, and bounded authorized outbound Meeting-context cases;
- the versioned translation corpus now covers contextual Meeting turns, modality/uncertainty, conditional meaning, quantifier scope, and spoken disfluency in addition to earlier literal/negation/technical/adversarial cases.

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
