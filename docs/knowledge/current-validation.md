---
id: document.knowledge.current-validation
class: DOCUMENT
domain: knowledge
role: CONTRACT
authority: CANONICAL
lifecycle: ACTIVE
---

# Current Validation

This file owns **proof interpretation**, not a run diary. Opt-in `tools/repository_verification.py` receipts may prove only the selected commands actually completed on the exact clean `Local` SHA. `SELECTED_CHECKS_PASS_PARTIAL_PROOF` leaves unknown dependency closure and other tests unverified; it is never release/native acceptance. `planning/development.md` owns continuation; GitHub remains authoritative for exact run/job metadata.

## Source Authority

Repository: `MIVUBI-STD/M-TranslateIT`

**Local-only development/source authority:** `Local` owns active development, governance, source proof, continuation, and release-source validation. The GitHub default branch `main` is reserved for accepted final releases; do not use its status or older workflows as current `Local` proof.

one SHA does not prove another SHA. Later documentation-only commits do not upgrade runtime/source proof.

## Current Source Proof

Use the latest completed matching verifier for the exact `Local` SHA and changed domain. Ancestor proof applies only to unchanged domains. Queued/running/skipped/cancelled/unrelated jobs are not PASS.

Do not preserve mutable “latest PASS” snapshots here. Historic GitHub Actions runs are not proof of the current `Local` commit.

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

Static Skill admission and procedure-case validation are repository-source checks only. `--score-procedures` validates the consistency of externally supplied agent receipts; it does not observe or verify agent decisions itself. Unrun checks remain unverified.

## Verification surfaces

Current policy: **GitHub source management only; no cloud/hosted execution**. The eight existing manual-only GitHub Actions definitions are retained as inactive source files and must not be dispatched. A workflow definition, historical status or queued job is not a current executable PASS.

```text
GitHub Local
→ canonical source ownership, stable docs, bridge contracts, dependency/lock inspection
→ static review and bounded candidate proof planning only

Repository Verify / Code Health / MiLMMT / ASR / TTS / Quality Readiness /
WorkerRuntime Lock / R3 Release
→ existing workflow definitions, not active runners
→ their results are NOT EXECUTED for current Local until actually observed

Single integrated local build/acceptance phase (only after source candidate admission)
→ npm/frontend checks and build, Cargo/Rust checks, Python worker tests
→ controlled package creation, install, native microphone/GPU/meeting evaluation
→ each claim requires an exact revision and actual measured output
```

There is no hosted CI fallback or required default-branch change. Source inspection is sufficient only for source claims; executable status stays unknown until the integrated local phase. Missing proof does not authorize fabricated CI, extra branches or repeated user-PC tests.

## Proof Boundaries

### REMOTE_GITHUB

Can establish source ownership, source-defined contracts, bounded history and static consistency only. No hosted execution is authorized.

It does **not** prove physical Windows microphone/device behavior, real CUDA throughput, installed-package behavior, meeting-app reception, audio fidelity, or practical latency.

### LOCAL_CODE

Can additionally prove the exact checkout/toolchain/filesystem/build that actually ran locally. It still does not automatically prove target-device acceptance.

## Target Windows

Native acceptance is not an incremental user-PC request. Build/package and native claims remain `UNKNOWN / NOT EXECUTED` until a source-integrated candidate reaches the grouped local build/install/test phase in `docs/knowledge/operations/target-windows-performance.md` and the relevant steps actually succeed.

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
