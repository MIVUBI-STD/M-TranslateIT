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

## Developer acceptance evidence — 2026-10-12

Fixed tested revision: `230ca94faae178659a7738fa82399485c267c991` on `Local`; initial requested candidate: `497041064aee4e797db57dc8a97a67974e5577aa`. Execution occurred in the user-authorized integrated developer phase on the target Windows PC. The user subsequently paused testing to continue source work in ChatGPT. This is partial acceptance, not a release or full-test completion. Documentation-only descendants retain these observations for unchanged runtime source; source changes require their own relevant proof.

### Environment and prepared inputs

Windows developer runtime, RTX 3070 8 GB, approximately 32 GB system RAM, NVIDIA driver 610.62; Node 22.23.3 in a dedicated test directory, Rust 1.96, Python 3.12.10, uv 0.12.5 and Visual Studio 2022 Build Tools. Initial `DEV.cmd doctor` and locked `DEV.cmd setup` passed. Existing dependency/model pins were preserved. Setup reported two high-severity npm advisories; remediation/exploitability was not established and no automatic dependency upgrade was performed.

| Input | Verified identity / preparation |
|---|---|
| Faster Whisper large-v3-turbo | `dropbox-dash/faster-whisper-large-v3-turbo` revision `0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf`; 7 manifest hashes verified |
| MiLMMT | `xiaomi-research/MiLMMT-46-1B-v1.0` revision `4fc480b6c58dec29c159dcdf9fde0f6d5c354995`; 10 manifest hashes verified |
| GPT-SoVITS | upstream source `d523079fc05d9a8028d6085bffe4a2757c32abb6`; pretrained snapshot `336b2ec4e8d4ac74740798dd40af44e74659ecaf`; 4 manifest checkpoint hashes verified, pinned HuBERT/RoBERTa resources staged |
| NLTK | data revision `550b6625bcef1f2abff2ff770a5a0d272c9c6b2a`; cmudict and both required tagger archives hash-verified and extracted |
| FFmpeg | BtbN `autobuild-2026-09-30-13-08`, archive SHA-256 `4a7642b2264c03e8a0ce8a3825b933ee5580656f45695a086fe7e294045ffc0a`; binary/version/LGPL provenance checks passed |
| Built-in voices | existing licensed Male/Female reference hashes verified; both actors exercised through the canonical selected-voice worker smoke |
| Device/data baseline | UI discovered Triton physical USB microphone and standard CABLE Output; discovery is not capture/delivery proof. Public ASR WAV and 20-prompt subsets per evaluator domain prepared locally; canonical full corpora unchanged. Optional ASR medium was not required/staged. No personal voice dataset supplied. |

Public pinned downloads succeeded with implicit Hugging Face token use disabled for that session after an expired-token authentication failure. No credentials were changed or exported. Engine assets and local settings are not included in Git source.

### Executed checks and live observations

| Scenario | Result | Evidence ceiling / limit |
|---|---|---|
| Repository verification and frontend/source contracts | PASS | EXECUTED_SOURCE on tested revision |
| Strict Svelte/typecheck | PASS | 0 errors, 0 warnings |
| Frontend runtime tests | PASS | 289 passed, 0 failed/skipped |
| Rust `cargo test --locked` | PASS | 205 passed, 0 failed |
| Worker Ruff/pytest | PASS | 161 passed; final worker contents match tested revision |
| Translation / ASR / TTS evaluator self-tests | PASS | 180 regression + 40 held-out / 60 / 30 cases; evaluator correctness only, not real quality scores |
| `DEV.cmd build` with expected SHA | PASS | frontend bundle and locked Cargo check; developer compile admission, not installer |
| Canonical persistent-worker CUDA smoke | PASS | actual ASR, complete ID→EN and EN→ID, intentional overlength rejection and persistent translation lifecycle |
| Built-in Male and Female synthesis | PASS | separate selected-voice smoke runs produced WAV artifacts on CUDA; auditory quality/playback not accepted |
| Setup defer → Text, checkpoint after restart | PASS | native developer UI observation |
| Male selection after restart; Male→Female | PASS | native UI selection and persisted actor identity; Female restart persistence not observed |
| Text ID↔EN, Swap | PASS | actual canonical results rendered in native UI for public prompts |
| Copy | PASS | UI copy followed by paste into source matched translated text |
| Clear / Undo | PASS | disposable public source and result emptied, then restored during app session |
| Focused Ctrl+Enter / Ctrl+K | PASS | translation triggered from focused editor; Quick Actions opened |
| Terms and style save | PASS | spoken term, preferred translation and Formal style save acknowledged; preferred English wording appeared in actual result |
| Alternative wording | PASS (bounded no-change case) | explicit no meaning-preserving alternative response preserved original result; a distinct successful alternative was not observed |
| Existing result invalidated after source/style change | PASS (limited) | Copy/wording/caption actions disabled for stale displayed result; in-flight stale response rejection not exercised |
| Settings restart persistence | NOT EXECUTED | save acknowledged in-session only; final restart not performed |
| Quick Translate / floating-caption delivery | NOT EXECUTED (attempt incomplete) | search/click attempted; later disabled command and completed 34-character translation trace observed, without caption result. Escape interrupted observation. No confirmed root cause or failure verdict. |
| Physical microphone Meeting, short/normal speech, Stop during processing | NOT EXECUTED | initial pending-readiness message only; no Meeting Start in final session |
| Google Meet route and separate receiver | NOT EXECUTED | no receiver observation; delivery unverified |
| Incoming captions, self-TTS suppression and outbound isolation | NOT EXECUTED | outbound prerequisite not accepted |
| My Voice recording, preview/cancel, training, 8 held-out samples, approval/reuse/recovery | NOT EXECUTED | authorized recording dataset and human listening/approval missing; built-in engine smoke does not prove training |
| Full keyboard/accessibility/minimum-size/DPI/caption hide-show/privacy matrix | NOT EXECUTED | only named interactions and Settings scrolling observed; no full visual/privacy acceptance |
| Cold/warm Start→Live, 10–20 utterance latency, five cycles, 30-minute stability | NOT EXECUTED | no session-level CPU/RAM/GPU/VRAM/drop/restart/capture/late-output measurements |
| Installer/signing/installed updater/clean-machine/main promotion/hosted CI | NOT EXECUTED — out of scope | developer execution proves none of these |

### Resolved admission failures and remaining diagnosis

The initial candidate did not pass all executable gates. Its first failing owners were corrected on `Local`, then relevant checks were repeated; earlier failed runs are not current PASS:

- `b07bc10c`, `4e24c81a`, `37549fe4`, `285b5043`: missing governance contracts, removed microphone diagnostic read, strict Svelte/Windows launcher errors and a capability validator ignoring separate main/overlay ownership.
- `c4d5cf2f`, `88945eea`, `5fa0fc21`, `cc4487b0`, `8fc1bed4`: stale frontend/worker assertions, worker lint, Rust built-in voice/preview references and shared audio/session fixture isolation.
- `0023e6ee`: Windows npm launcher EINVAL; canonical build now invokes the current Node npm CLI directly.
- `c4145afd`: Tauri updater startup null-configuration panic; inert developer config supplied while release signing gates remain unchanged.
- `e6b34865`, `d3a54b4a`, `230ca94f`: upstream output polluting JSON stdout, reference decoding demanding absent TorchCodec, its stale static protocol assertion and bundled NLTK resources absent from the cached search path. Canonical headless shims isolate provider output, use locked SoundFile reference decoding and register the pinned NLTK path; no upstream/model/dependency substitution.

Do not infer a Quick Translate defect solely from the interrupted UI attempt. Inspect `src/components/runtime/ProductivityActions.svelte`, `CommandPalette.svelte`, `src/app/runtime/translationOverlayRuntime.ts` and their existing bridge/Rust consumers under `EngineData/Frontend/RustApp` to separate clipboard access, command execution, busy/readiness mapping and overlay publication. The early Meeting session-readiness notice likewise remains an observation, not a diagnosed capture failure. Native testing stays paused per the user; continuation belongs to `planning/development.md`.

### Private evidence retention

Raw logs, local paths, settings and media remain outside tracked source. Locally retained ignored receipts include `.cache/acceptance-source-230ca94f.log`, `acceptance-cargo-230ca94f.log`, `acceptance-repo-230ca94f.log`, `acceptance-build-230ca94f.log`, `acceptance-nltk-root-worker.log`, the three `acceptance-*-self-test.log` files, separate Male/Female smoke logs, and `acceptance-text-settings-run-230ca94f.log`. The local baseline summary is `.cache/acceptance-result-230ca94f.json`; final interrupted UI observations are recorded in the fixed-revision table above. Public prompt inventory is `.cache/acceptance-test-data/engine-and-public-data-230ca94f.json`. `UserData/LogData/RustAppValidation/latest_worker_smoke_result.json` is mutable and was overwritten by Female; separate smoke logs preserve both runs. These paths are retrieval hints on the tested PC, not downloadable GitHub proof artifacts. No conversations, recordings, credentials or raw machine paths were committed.

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
