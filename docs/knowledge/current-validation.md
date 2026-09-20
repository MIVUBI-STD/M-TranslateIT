# Current Validation

This file owns **proof interpretation**, not a per-run diary. `next-action.md` owns continuation; GitHub remains authoritative for exact run/job metadata.

## Source Authority

Repository: `MIVUBI-STD/M-TranslateIT`

**Local-only source authority:** `Local` is the sole active branch for development, governance, CI, proof, continuation, and release-source validation.

one SHA does not prove another SHA. A later documentation-only commit may record proof without changing the validated runtime/source identity.

## Current Source Proof

Exact proof identity is read from GitHub Actions for the exact `Local` SHA and changed domain under discussion. This file does not pin a mutable "current source SHA", because doing so becomes stale as soon as `Local` advances.

Interpretation: use the latest completed matching verifier for the changed domain. Ancestor proof applies only to unchanged domains. Documentation-only commits create no runtime proof. Queued/running/skipped/cancelled/unrelated jobs are not PASS.

Recent verified baseline relevant to the current work:

```text
Code Health                 a26d5e33b31eee6753ff1bc7f09b847f11770bc3  PASS
MiLMMT Repository Contract  848329bb21b45d72eccde0a19160490c9d843f22  PASS
```

Later translation-quality commits require their own MiLMMT Repository Contract result. GitHub is authoritative for exact run/job IDs.

## Current Source Claims

Subject to matching proof for the changed domain, current source establishes:

- one generation-bound Meeting lifecycle authority with fail-closed Start/Stop, cleanup, route binding, stale-work rejection, and bounded helper recovery;
- explicit owners for Meeting state/consumers/pipelines, helper scheduling/transport/readiness, My Voice recording/build/inference, storage/promotion, and Windows audio;
- one canonical MiLMMT ID↔EN pipeline, explicit degraded CPU fallback only for known capability absence, and no silent cloud/parallel fallback;
- generation-bound functional readiness, bounded outbound context, at-most-once output, and explicit incoming degradation;
- latency hardening through bounded capture, finalized-WAV preparation, deterministic KV-cached MiLMMT generation, warm actor reuse, and reference-speaker embedding caching;
- Meeting Diagnostics with queue/drop plus translation tokenize/inference/decode/throughput telemetry;
- translation-quality tooling with baseline-vs-candidate critical regression detection, grouped critical-pass-rate deltas, authorized Meeting-context requests, and a 170-case semantic-risk regression corpus plus a separate 40-case held-out benchmark with risk-tag gates;
- ASR quality tooling with a 60-case linguistic/accent/acoustic/device/meeting-compression corpus, WER/CER, grouped critical rates, matched provenance, and baseline-vs-candidate regression comparison;
- TTS/My Voice quality tooling with a 30-case synthesis acceptance corpus, speaker-similarity/intelligibility/artifact evidence, matched provenance, and fail-closed regression comparison;
- Quality Readiness aggregates Translation + ASR + TTS comparison evidence without recalculating domain metrics, verifies expected candidate identities and input-report SHA-256, and fails closed if any domain is incomplete or regressed;
- My Voice live inference isolated from one-shot dataset/training/evaluation/package construction.

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

ASR Quality Contract
→ ASR corpus/evaluator + canonical ASR source contract

TTS Quality Contract
→ TTS/My Voice corpus/evaluator + canonical held-out voice-evaluation source contract

Quality Readiness Contract
→ cross-domain aggregation contract over Translation + ASR + TTS comparison reports

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
