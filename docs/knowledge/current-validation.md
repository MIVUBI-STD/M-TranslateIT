# Current Validation

This file owns **proof interpretation**, not a run diary. `next-action.md` owns continuation; GitHub remains authoritative for exact run/job metadata.

## Source Authority

Repository: `MIVUBI-STD/M-TranslateIT`

**Local-only source authority:** `Local` is the sole active branch for development, governance, CI, proof, continuation, and release-source validation.

one SHA does not prove another SHA. Later documentation-only commits do not upgrade runtime/source proof.

## Current Source Proof

Use the latest completed matching verifier for the exact `Local` SHA and changed domain. Ancestor proof applies only to unchanged domains. Queued/running/skipped/cancelled/unrelated jobs are not PASS.

Do not preserve mutable “latest PASS” SHA snapshots here; read GitHub Actions for exact run/job identity.

## Current Source Claims

Subject to matching proof, current source establishes:

- one generation-bound Meeting lifecycle authority with fail-closed Start/Stop, cleanup, route binding, stale-work rejection, bounded helper recovery, and at-most-once output;
- explicit ownership for Meeting, Mic Test, My Voice, helper scheduling/transport/readiness, Windows audio, storage/promotion, and shared-resource arbitration;
- one canonical MiLMMT ID↔EN pipeline with explicit bounded degraded fallback and no silent cloud/parallel fallback;
- bounded capture/context/queues/transcripts/diagnostics/incidents/cache/feedback/temp data;
- latency hardening through bounded finalized-audio preparation, deterministic MiLMMT generation, warm actor reuse, and reference-speaker embedding caching;
- quality tooling for Translation, ASR, and TTS/My Voice with baseline-vs-candidate regression contracts plus cross-domain Quality Readiness aggregation;
- one-shot signed updater source contract with no updater polling daemon;
- bounded productivity contracts: Quick Translate, temporary last-session review, Meeting presets, terminology maintenance, in-memory Text cache, and opt-in translation feedback;
- application-level coordination source contracts: typed ApplicationRuntime snapshot/intent/mutation boundaries, distinct Meeting/Mic Test/My Voice owners, event-first Meeting reconciliation with slow fallback, centralized shutdown, modular frontend controllers, split facades/state policies, and guarded compatibility surfaces.

## Verification surfaces

```text
Repository Verify
→ governance / Local-only routing / repository contracts

Code Health
→ push / pull_request / workflow_dispatch
→ manual dispatch = full-domain Frontend + Rust + Python proof
→ frontend: typecheck, build, runtime tests, bridge/source-size/reachability/dependency/package contracts
→ Rust: compiler/dead-code, Clippy, unit tests on Linux + hosted Windows
→ Python: compile, Ruff/static/format, pytest on Linux + hosted Windows
→ exact-SHA aggregate summary; skipped domains are non-evidence

MiLMMT Repository Contract
→ canonical translation repository contract + exact-SHA summary

ASR Quality Contract
→ ASR corpus/evaluator/runtime contract + exact-SHA summary

TTS Quality Contract
→ TTS/My Voice corpus/evaluator/held-out contract + exact-SHA summary

Quality Readiness Contract
→ cross-domain Translation + ASR + TTS aggregation + exact-SHA summary

WorkerRuntime Lock Consistency
→ Python dependency-lock integrity + exact-SHA summary

R3 Release Contract
→ release-source contract
→ manual dispatch explicitly requires controlled Windows payload proof
```

Path-targeted skipped jobs are not evidence for unrelated domains.

## Proof Boundaries

### REMOTE_GITHUB

Can prove source ownership, contracts, hosted execution, bounded recovery behavior, and matching CI tests.

It does **not** prove physical Windows microphone/device behavior, real CUDA throughput, installed-package behavior, meeting-app reception, audio fidelity, or practical latency.

### LOCAL_CODE

Can additionally prove the exact checkout/toolchain/filesystem/build that actually ran locally. It still does not automatically prove target-device acceptance.

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
