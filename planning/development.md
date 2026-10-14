# Active development — My Voice source stabilization (2026-10-14)

**State:** SOURCE_CANDIDATE — UI_UX_SOURCE_REFINEMENT / EXECUTION_NOT_RUN. The previously frozen backend source scope stays unchanged while the user-approved My Voice frontend layout receives a bounded UX pass; no UI, native, sound-quality or installed-product acceptance has been executed.

**Authority:** `MIVUBI-STD/M-TranslateIT`, branch `Local`. Pin the actual `Local` HEAD for each read/write and all future executable acceptance. The 2026-10-12 partial developer checks on `230ca94faae178659a7738fa82399485c267c991` do not certify newer My Voice revisions.

## Current scope and source truth

- Preserve the current Tauri/Svelte + Rust storage owner + one Python worker, GPT-SoVITS V2ProPlus revision, 600 guided English prompts and the **128-take maximum per build**. The larger prompt catalog is not a promise to train 600 takes at once.
- Keep ordinary recording, Quick Preview, frozen-dataset Create, eight held-out review samples, explicit approval/Meeting selection, saved-version Improve (rebuild), and immutable backup as the first-release target.
- Preserve previous saved recordings and selected Meeting voice; unknown storage/worker state fails closed. Backup verification requires receipt-to-payload **and payload-to-receipt** linkage and content-addressed receipt identity; unreferenced archive bytes are not restorable objects.
- Ambiguous Start/Stop/Retry/Accept recording outcomes keep recording coordination locked until a fresh Rust status read. Accepted-take consumers refresh after uncertain Accept without replaying its write.
- Uncertain Create, Stop Creating and Approve responses freeze My Voice mutations until a fresh Rust build status is read. Ambiguous Approve also refreshes canonical Meeting selection without repeating approval or falsely claiming success.
- My Voice view now rejects out-of-order polling reads (including failed reads), invalidates pending reads before explicit mutations, and prevents a verified mutation result from being overwritten by an older status. This is presentation-level sequencing, not an alternate Rust state owner.
- Quick Preview and held-out evaluation audio reads use per-view request identity, and completion/error callbacks validate the current audio instance before touching review evidence. Stop/dispose/review change invalidates old reads and detaches old callbacks.
- Saved-version selection invalidates stale confirmation on failed or changed version inventory, blocks changes while the inventory is unreadable, and checks native Meeting voice after an uncertain restore without replaying that mutation. Selection reads are ordered, and a personal voice is not announced as selected when status cannot verify it.
- Source contract audit repaired two stale Quick Preview assertions: the displayed prompt total follows the 600-line catalog dynamically while the build cap stays at 128; held-out `Listened` proof follows the current audio instance/generation/review ID rather than the previous callback shape. The assertions were inspected, not executed.
- Exact selected-version identity uses an **explicit, read-only** Rust verifier: it streams SHA-256 over the actual selected actor package and compares its content-addressed receipt digest to the requested version ID. No heavyweight hash enters frequent status polling. Ambiguous restores stay locked when identity is **unknown**, but an explicitly verified mismatch (`false`) releases the lock without claiming the requested version was restored. This distinguishes match/mismatch/unavailable and avoids permanent lockout when the earlier Meeting voice is intact. Native execution remains pending.
- The explicit exact-version hash command now runs on Tauri's blocking worker instead of the UI thread, and rechecks Meeting/recording/build/Quick Preview ownership after acquiring the existing voice-selection guard. No heavy hashing is added to status polling. Node regression coverage is authored but unexecuted.
- Backup integrity now also binds the original immutable `DatasetSnapshots/<sha>.json` and `TrainingLineage/<sha>.json` path IDs to the SHA-256 of their actual content. This closes a gap where the backup's own index could be updated to match forged archive metadata, while the immutable history store would later reject the imported object. Added negative Rust fixtures; execution remains untested.
- Backup semantic validation now checks each frozen DatasetSnapshot's original typed source manifest (authorization, take IDs/names and held-out separation), its audio-link cardinality, and each link's exact archive receipt line/text binding. TrainingLineage metadata must use the supported schema and engine revision before its references can pass. These source-only regressions are authored, not executed; the backup import still never activates a Meeting actor.
- VoiceVersions backup verification now additionally checks the immutable actor receipt schema/revision, recorded payload sizes, and the existing Rust actor package validator before declaring a version restorable. This avoids accepting self-consistent backup hashes that could not be activated as MyVoice. Regression fixtures for unsupported version receipts are authored; native verification is unexecuted.
- Create → Review → Approve promotion now requires the staged and promoted actor's actual SHA-256 package identity to equal the immutable candidate version preserved before replacing the selected Meeting actor. A mismatch before replacement leaves the current actor untouched; a mismatch afterward uses the existing rollback path. Rust and source-regression fixtures are authored but NOT EXECUTED.
- Developer-only GPT/SoVITS checkpoint probes remain isolated. User Resume remains **DISABLED** because SoVITS scaler/scheduler continuity, cross-worker provenance and actual Windows GPU restoration are not established. Avoid expanding this experiment while the primary flow lacks native acceptance.
- `docs/knowledge/current-validation.md` owns evidence interpretation. No exact-`Local`-HEAD Svelte/Node/Cargo/Python build/test, real personal-voice training, Windows backup recovery, perceptual voice comparison or Meeting receiver proof is available for this source batch.

## Final bounded source admission — 2026-10-14

- **Reviewed code authority:** `Local` source HEAD `fe5647249bfdd420693630fd48acac96da046c0a`; this decision record is documentation-only. The next real local run MUST externally pin its own actual 40-character `Local` commit SHA after this note is committed.
- **Inspected boundaries:** guided recording -> frozen Rust dataset -> one Python `voice_lab_build.py` worker CLI -> held-out review identity -> explicit user quality consent -> immutable candidate fingerprint -> selected Meeting actor with rollback; archive/backup schema, content identity, training snapshot prompt binding and VoiceVersions package admission.
- **Regression-contract spot-checks:** direct source-regex assertions in `voice_build_runtime_sync_contract.test.ts`, `voice_version_restore_contract.test.ts`, `voice_event_refresh_contract.test.ts`, and `voice_quick_preview_isolation_contract.test.ts` showed no confirmed source mismatch after respecting variable shadowing inside individual test scopes. This was manual/static inspection, **NOT** test execution, not complete AST/typechecking, and not proof of every dynamic assertion. Rust -> Python CLI flags were compared with the Python argparse declarations; no contract mismatch observed in the inspected arguments.
- **No known source-proven blocker in this bounded review.** Do not add another source patch without fresh, reproducible source or exact-SHA execution evidence. Unexecuted Svelte/Node/Rust/Python tests, native file promotion/rollback, target GPU model performance, actual voice quality, VB-CABLE/Meeting reception, backup recovery, and full installed-product acceptance all remain **UNKNOWN / NOT EXECUTED**.
- **Existing single acceptance owner:** `docs/knowledge/operations/target-windows-performance.md`. Keep the prescribed one-SHA order: verify clean checkout and Node/Rust/Python/uv versions and locks; `npm ci`, `npm run validate:source-contracts`, `npm run typecheck`, `npm run test:frontend-runtime` plus pinned Python tests; `DEV.cmd build` with externally pinned `TRANSLATEIT_EXPECTED_SHA`; only after successful compiler/source gates, `DEV.cmd run` and controlled native My Voice / Meeting / backup scenarios. STOP and record the first failure, never skip to audio acceptance. No hosted CI, fabricated receipts, or piecemeal PC testing.

## User-approved My Voice UI/UX refinement — 2026-10-14

- Compact current Meeting voice selector remains at top with the **same** BuiltinVoicePicker handlers and Rust-owned selected-voice verification. No Tauri commands, selection logic, models, or voice archive formats change.
- Prioritize the Record → Create → Review workspace directly after current voice status; collapse secondary Saved Recordings / Voice Versions / Storage & Backup and earlier-recording protection below the primary workflow. Stage labels become accessible navigation, including an explicit Return to Review from recording when a review candidate remains available.
- Show Loading audio during the verification/download of Quick Preview and held-out samples, with request-identity fencing and playback/listening integrity preserved. Avoid repeated Quick Preview explanation; expose a complete immutable version ID on demand without fabricating a version date.
- Manual backup folder path stays because the current native dialog plugin is not part of the already installed runtime; do not add a new Tauri dependency or pretend to have a folder-picker integration in a source-only UI pass.
- These Svelte/Node test assertions are **authored only**. Layout, keyboard navigation, responsive rendering and accessibility must be assessed on an actually running Windows build at the same exact SHA; STATIC REVIEW IS NOT A VISUAL PASS.
- The existing integrated U4/A9 Windows acceptance runbook now has bounded My Voice UI checks for the compact current-voice picker, workspace precedence, keyboard-accessible stage switching/Return to Review, pending audio indicators with stale callback handling, and saved-tool disclosures. This is an acceptance plan update, **NOT EXECUTED**; no new test harness, backend selection logic or model changes.

## Source addition — My Voice compute and actual-stage visibility (2026-10-14)

- Explicit-on-entry/recheck CPU/CUDA advisory uses the selected local Python runtime via a bounded, asynchronous Rust-owned probe. It is NOT added to the frequent build-status poll. CUDA detection, reported total VRAM and CPU-only mode are not proof of training success or realtime performance. Missing Python/PyTorch, failed CUDA inspection, malformed output or timeout show unavailable.
- My Voice Create controls wait for a completed probe; CPU-only training additionally requires informed opt-in because this pinned upstream CPU path is unverified. This UI gate does not claim to replace backend engine admission or CPU/driver validation. GPU users are told detection does not establish free VRAM or acoustic quality.
- GPT-SoVITS emits explicit worker stage messages before dataset feature extraction, SoVITS training and GPT training, then the existing held-out evaluation stage. The UI displays authoritative child messages without invented percentages, ETAs or epoch accounting. Rust owns the existing build lifecycle and cancellation.
- The small-resource release map and validators include the new explicit hardware probe. Regression tests are authored for probe states, wiring and stage ordering; **not executed** in this GitHub-only pass. Exact-HEAD compile, Windows CUDA/CPU paths, responsive render, checkpoint safety, training audio/Meeting and clean-machine installation remain **unverified**.

## User-requested safe RTX 3070 8 GB design — 2026-10-14

- The user explicitly declined validation for now and requested memory-safe, crash-contained custom-voice generation focused on the RTX 3070 8 GB. 12 GB is optional margin, not a product minimum.
- Drafted the bounded `docs/knowledge/proposals/2026-10-14-my-voice-safe-8gb-training.md` source-informed RFC. This is a **proposal**, not an implemented feature, passed test or verified hardware capability.
- Source risks: total-VRAM-derived training batch (8 GB can yield batch 4), no actual free-VRAM admission, stages executed through non-supervised subprocesses, one-GPU Windows DDP native-crash exposure, evaluation model lifetime, partial checkpoint/cancellation risks.
- Next engineering work is code safeguards and integrity before any test authorization. Preserve original user audio, immutable versions, pinned engine/model source, one Rust authority, and existing Meeting voice selection throughout.

## SAFE_8GB source foundation — 2026-10-14

- Added provisional GPU free/total headroom calculation, original admission budget and CPU-only block through one Python policy owner. Rust applies a native Create/Improve guard **before** dataset freeze and passes the bounded budget to training; UI reflects the admission result. This is source code, not hardware proof.
- Physical SoVITS/GPT batch is now 1 rather than deriving batch 4 from total 8 GB. Stage runner rechecks free GPU memory ahead of each heavy stage and applies a PyTorch allocator fraction in its own process. Windows SoVITS/GPT spawned children, non-PyTorch allocations and external GPU users are **NOT guaranteed capped**; full process containment and one-GPU DDP compatibility remain open.
- Initial 8 GB reserve/budget thresholds are hypotheses and may reject training or still permit OOM. No Windows, GPU, model or test execution performed; do not claim RTX 3070 compatibility.
- Next: implement an explicitly versioned Windows single-GPU trainer adapter / spawn-child allocator coverage and owned process tree, then integrity-bound per-run status and recovery. Do not allow partial GPU guard to imply zero-crash safety.

## Single-GPU trainer adapter source — 2026-10-14

- Added a **revision/hash-bound**, fail-closed GPT-SoVITS `s2_train.py` / `s1_train.py` in-memory adapter. SoVITS executes single-GPU `run(0,1,hps)` without `mp.spawn`, Gloo group initialization or DDP wrapping. GPT Lightning uses `devices=1` and `strategy="auto"`. The original pinned upstream files remain unchanged.
- The stage runner installs its own CUDA allocator budget before loading verified upstream code. The adapter only applies if the exact Git blob hashes match the source revision pinned in this project; unknown source/patch shape aborts rather than training with unreviewed semantics.
- **This is still UNVALIDATED SOURCE.** Windows loader/source-layout compatibility, optimizer/checkpoint semantics, training quality, data-loader children and CUDA native stability are unknown. No test execution requested/attempted. Pending: stronger Windows process-tree ownership, stage telemetry, per-run status generation and verification of every output before approval.

## My Voice bounded stage supervisor source (2026-10-14)

- Added Windows Job Object `KILL_ON_JOB_CLOSE` ownership around every upstream stage child. Training refuses unowned or unaccounted Windows process trees. This does not eliminate the brief process creation-to-job-assignment race; shutdown still requires native scrutiny.
- An independent nvidia-smi free-VRAM sensor checks every CUDA stage periodically. Sustained critical VRAM (<768 MiB) or repeated sensor failure aborts the owned stage; downstream model approval remains guarded by existing review rules. The threshold is provisional and **not benchmarked**.
- Source regression fixtures authored only. No Windows compile, GPU run, audio quality test or test execution authorized. Next: generation-bound status/completion receipts and GPU evaluation protection.

## Run-scoped source integrity — 2026-10-14

- Added Rust-owned `run_identity.json` binding each frozen dataset SHA-256 to a unique run token/generation. The Python worker writes monotonically sequenced, token-bound status updates; Rust refuses stale/mismatched/invalid phases.
- `ready_for_review` is no longer trusted alone. Only successful child exit, actual evaluation manifest/recording checks, and a new create-only Rust `completion.json` enable a reviewable candidate. Failed/interrupted runs remove completion evidence and preserve existing training recovery checks.
- Existing approved personal actor and saved take archives remain unchanged; this is a source-only change, no compilation or runtime proof.

## Isolated per-candidate GPU evaluation source — 2026-10-14

- Held-out candidate evaluation now uses one GPU child process per checkpoint candidate, each under the owned stage supervisor and fresh prelaunch budget check. The parent validates the candidate identity, expected checkpoint paths, exact eight held-out lines and SHA-256 of generated WAVs before ranking or promotion.
- The evaluator applies a per-process CUDA allocator fraction before TTS model loading; GPU memory is released by child exit rather than relying only on `empty_cache()`. No automatic retry or fallback is introduced.
- This is source-authored work. No test, GPU inference, human voice quality listening, native build or Windows Job execution has occurred. Further source review needed for load/driver failure and restart behavior.

## Native Python build-tree owner source — 2026-10-14

- Added a Windows-only Rust Job Object owner for the top-level `voice_lab_build.py` process. Assignment and KILL_ON_JOB_CLOSE survive UI closure and desktop termination (assuming the process was assigned). Cancellation uses the owned Job rather than arbitrary process enumeration, and declines to call the build stopped until the worker reconciles.
- Windows Job APIs are invoked through a scoped internal Rust module, no new Cargo dependency/lock edits. Assignment occurs just after `Command::spawn` (initial race remains). Existing trusted checkpoint preservation runs after subprocess exit.
- No cargo build, Windows runtime, native process lifecycle or validation was executed. Product safety remains unverified.

## Suspended Windows process admission — 2026-10-14

- Both top-level My Voice Python build and upstream stage children now request Windows `CREATE_SUSPENDED`, obtain the initial thread via Toolhelp32, assign the suspended child to a kill-on-close Job Object and resume only after successful admission. A failure or ambiguous thread enumeration terminates only the owned job.
- These wrappers use the existing Windows APIs rather than a new dependency; no tests or native verification executed. Open question: process handles, nested job eligibility, service restrictions and driver-exception behavior remain unverified until explicitly authorized. This closes the intended user-code-before-job-assignment gap in source, not an audited Windows guarantee.
- Next focus: RAM/disk pressure and preventing checkpoint copies without adequate real free space.

## Verification boundary

This phase is `REMOTE_GITHUB` only: review exact source and write bounded regression tests; do not dispatch hosted CI, claim test execution, request incremental PC testing or touch `main`. Once the integrated source candidate is coherent, the existing `docs/knowledge/operations/target-windows-performance.md` will govern a single later local build/package/Windows acceptance phase.

## Single next step

**User-requested 8 GB safe training architecture comes first.** Review and implement the bounded proposal at `docs/knowledge/proposals/2026-10-14-my-voice-safe-8gb-training.md`: native Create/Improve admission, per-stage CUDA memory budget and batch=1, explicit single-GPU DDP compatibility decision, isolated child lifecycle, and checkpoint/output integrity. **No validation, tests, hosted CI, native training, release or GPU experiment is authorized in this design phase.** Existing Windows acceptance runbook remains deferred until user explicitly requests it; no premature promotion to `main`.

## Deferred prior work

Meeting V2 Phase 0 remains a separate unvalidated source candidate. Its continuation and evidence limits are preserved in prior `Local` commits and `docs/knowledge/operations/target-windows-performance.md`; do not treat older Meeting proof as acceptance of My Voice or silently resume Meeting work during this My Voice pass.
