# Active development — My Voice SAFE_CUDA device-aware training

**State:** SOURCE_CANDIDATE / NOT EXECUTED. **Scope:** device-aware Windows CUDA training across supported NVIDIA GPUs, with RTX 3070 8 GB as one reference device, one GPT-SoVITS V2ProPlus training pipeline. This document records current intention, not runtime proof. Earlier implementation history stays in `Local` commits and the detailed reference `docs/knowledge/proposals/2026-10-14-my-voice-safe-8gb-training.md`.

## Canonical source ownership

- **Rust/Tauri:** sole Create/Improve resource authority, Meeting exclusion, run identity and completion receipts, native memory/disk admission, Windows build-process Job Object, final evaluation/approved-voice promotion and recovery archive.
- **Compute capability admission:** the selected PyTorch CUDA device must disclose its compute capability. The pinned mixed-FP16 trainer policy currently admits architecture major >=7 and explicitly blocks older known architectures rather than silently converting to more memory-demanding FP32. This is a provisional engine policy, NOT a universal claim that older GPUs cannot perform FP16. PyTorch's installed CUDA kernel architecture support also remains to be checked at acceptance.
- **Device selection:** enumerate PyTorch CUDA-visible GPUs; obtain a **single bounded NVIDIA inventory** with each device's physical UUID and total/free memory; skip individually unverifiable devices; choose the largest safely admitted current budget; then bind the physical UUID through Rust, Python, pinned SoVITS/GPT loader and live monitoring. CUDA ordinal `0` is only a **local index after UUID scoping**, not an assumption that all users train on physical GPU 0. Multiple GPUs can be discovered, but only one selected GPU is used per training run.
- **Zero-waste runtime:** stage heartbeat is emitted through the same run-token-bound Python status writer at most every 30 seconds during supervised training, with no invented progress. One Rust completion owner now also handles manual stop deadline using its existing child lifetime; no new background worker, scheduler or polling channel. A stop request during dataset preparation prevents Python training launch.
- **Python:** one existing local training coordinator; `my_voice_gpu_policy.py` (preflight arithmetic), `my_voice_adaptive_safety.py` (pure NORMAL/WATCH/PRESSURE/CRITICAL trend rules), `my_voice_stage_supervisor.py` (owned child lifecycle and telemetry), `my_voice_cooperative.py` (scoped checkpoint-boundary request/ack), `my_voice_single_gpu_adapter.py` (pinned SoVITS/GPT single-GPU compatibility). No second runtime/provider.
- **Svelte:** presentation only; read existing Rust build snapshot and safely bound hardware advisory. No independent training state writer or invented percent/ETA.
- **Pinned upstream:** GPT-SoVITS V2ProPlus source/model revisions remain unchanged on disk; adapter source-hash mismatch fails closed. The English-only actor/evaluation contract and eight held-out reviews remain unchanged.

## Device-aware checkpoint hardening

- Full SoVITS G/D optimizer checkpoint writes now use a source-hash-bound in-memory adapter to `utils.my_save`. Write into same-directory `.pending-*.partial`, flush and atomically publish a distinct checkpoint name; existing complete G/D files must not be overwritten. The generator/discriminator **pair** is still not an atomic transaction and is not automatically resumable.
- SoVITS inference candidate weights now use the pinned `process_ckpt.my_save2` binary serialization with same-directory atomic publication, preserving the original version header. A truncated/failed pending file is never published as a candidate.
- Disable `if_save_latest` pruning for SoVITS and GPT to retain older full checkpoints as candidate evidence across a failed newer write. The original GPT-SoVITS source files and model weights remain unchanged. Partial SoVITS optimizer checkpoint remnants block removal of the work directory pending manual attention.
- GPT Lightning full optimizer checkpoint writes now use a scoped `TorchCheckpointIO` plugin with same-directory staging and atomic replacement; GPT inference weights use the same torch serialization but a separate create-only atomic writer. A failed write leaves a `.pending-gpt-*` attention file so recovery blocks workspace deletion. The adapter fail-closes unless the installed package is the `pytorch-lightning==2.6.5` revision in `uv.lock`; deletion requests do not remove older valid checkpoint files. Critically, GPT safe-stop acknowledgment now happens from the **same pinned ModelCheckpoint callback after its save completes**, not from a second callback whose ordering is uncertain. The saved checkpoint must match the actual current epoch filename. Runtime behavior remains unverified.
- The revision-pinned SoVITS training adapter now publishes a same-directory SHA-256 pair receipt only after G/D optimizer checkpoints have completed for the same global step. Recovery validates paired receipt identities and file hashes, and preserves the receipt alongside checkpoint candidates. Legacy unpaired checkpoints remain evidence only. No optimizer/scheduler/GradScaler resume is authorized. Tests authored only, not executed.

## Source-candidate behaviors

1. **Admission:** native Create and Improve refuse unknown/insufficient current CUDA headroom or native RAM/disk readings. Provisional safe profile starts with physical batch size 1 for SoVITS and GPT, sequential GPU stages, and a bounded PyTorch allocator fraction (not a global VRAM ceiling). CPU fallback is not silently enabled.
2. **Isolation:** Windows build and stage children request suspended launch -> Job Object assignment -> resume. KILL_ON_JOB_CLOSE and stage descendant accounting prevent knowingly continuing with orphaned work. Windows compatibility is unverified.
3. **Adaptation:** high but stable usage => WATCH/continue. Soft resource decline during model allocation warmup does not immediately stop the trainer: an initial provisional 90-second stabilization and three sustained PRESSURE readings are needed before requesting a checkpoint-boundary yield. CRITICAL/UNKNOWN conditions override stabilization. SoVITS/GPT adapter honors a cooperative request only at an eligible checkpoint epoch. State changes are reported through the **existing** status writer, not every sample.
4. **Manual Stop and Emergency:** Stop Creating now creates a generation+run-bound, create-only signal; the **existing** stage supervisor requests checkpoint-boundary cooperative stopping during SoVITS/GPT, while non-training stages terminate immediately. The existing Rust completion-owner thread checks process exit once per second and provides a 30-second cleanup margin after the Python supervisor's provisional five-minute stage stop deadline before terminating the owned tree. Consecutive critical or unknown resource readings still terminate owned processes without waiting. Missing acknowledgments/crashes cannot silently become successful training.
5. **Integrity:** unique generation+dataset identity, monotonic worker phase, verified completion receipt after actual child success, isolated candidate evaluation and held-out SHA-256 checks. Training failure never automatically promotes a candidate or switches the Meeting actor.
6. **Recovery:** incomplete training checkpoint files remain **unverified candidates** and are copied only with measured free disk capacity. No automatic optimizer resume, cloud training, checkpoint deletion, release-ready declaration or hidden retry.

## Explicit limitations

- This is **source written to GitHub only**. No Rust/Node/Python suite, static checker, target-Windows compilation, CUDA/GPU run, generated voice listening, Meeting end-to-end test, packaging/install, fault injection or backup restore was executed during the requested source-only work. Source comparisons are not acceptance tests.
- RTX 3070 8 GB is a reference case, not a hardware-specific code path. GPU capability and free capacity determine admission; 12 GB is optional headroom, not a mandatory minimum. CPU-only, AMD/Intel GPU and Apple accelerator My Voice training remain unsupported/unverified by the current NVIDIA CUDA engine; detection alone is not compatibility. Provisional VRAM/RAM/disk thresholds may still block viable work or fail to prevent OOM; actual safe operating envelopes remain unknown.
- Cooperative stop now applies both to safety pressure and manual `Stop Creating`, but preserves at most an **unverified checkpoint candidate**; model/optimizer/scheduler/AMP continuity is **not resume-certified**. On device failure, lost telemetry or a missed bounded stop deadline, an owned hard stop may still discard unsaved work. Zero wasted effort cannot be guaranteed without proven optimizer resume.
- CUDA and NVIDIA global free/total memory are now source-bound to the same physical GPU UUID; PyTorch process-specific allocated/reserved/peak usage and runtime cross-process provenance are not yet fully integrated. GPU driver faults and sudden OOM can occur between sensor readings. Scoped SoVITS/GPT writer adapters now stage and atomically publish *individual* files, but a full G/D pair is not a single transaction and optimizer resume remains uncertified.
- Initial stage process ownership, nested Job Object behavior, Lightning callback order, pinned source adapter runtime behavior and low-memory pipeline semantics all require future Windows evidence. Never claim zero-crash safety.

## Source-only follow-up: wasted-work and wait-error barriers

- Fixed a Rust lifecycle case where an error from `child.try_wait()` could be hidden if a later `child.wait()` succeeded; child polling error must remain a failed build, never allow candidate promotion.
- Added generation-bound manual Stop checks at CPU-side boundaries: before workdir creation, feature extraction, SoVITS/GPT training, held-out evaluation, each new evaluation candidate, and candidate promotion. This reuses one existing stop file rather than extra worker/threads and prevents avoidable GPU launches after Stop.
- These are source changes only. Checkpoint integrity, native Windows behavior, Lightning plugin API and generation races remain unproven until later explicit validation.

## Source-only follow-up: bounded optimizer checkpoint budget

- Identified an internal count mismatch: saving full GPT optimizer state every epoch could produce at least 15 checkpoint files plus last-version variants, exceeding the Rust recovery group limit of eight and blocking recovery on a failed run.
- GPT full checkpoint saving no longer depends on the optional `top_3_acc` metric: the pinned `ModelCheckpoint` has `save_top_k=-1` and `monitor=None`, so each scheduled epoch's full optimizer checkpoint can be written without a missing metric suppressing the save. This is separate from the held-out WER/similarity candidate ranking.
- Reduced full-checkpoint frequency to one generic bounded budget: up to four SoVITS G/D paired saves and five GPT full-save epochs across the fixed training schedule (approximately every 2 SoVITS epochs or 3 GPT epochs). The pinned Lightning ModelCheckpoint now sets `save_last=False`, suppressing duplicate full-size `last*.ckpt` copies while preserving versioned epoch-specific full checkpoints. Planned source inventory is at most 4×3 SoVITS optimizer/pair artifacts + 5 GPT optimizer checkpoints + 2 configs = **19 items**, under Rust's 28-file overall and 12/group bounds. The existing 48 GiB total archive limit and measured disk admission are **not relaxed**. This trades slightly longer intervals between checkpoints for far less disk I/O; optimizer resume remains unavailable.
- Repaired authored SoVITS cooperative-stop fixture to require a hash-bound G/D pair receipt, rather than merely two arbitrarily named nonempty files. No tests or GPU stages were executed.

## Run-bound checkpoint evidence (source-only)

- The same existing stage-supervisor loop can now read one tiny run-scoped, atomically replaced `epoch_progress.json` **only when a full checkpoint is saved**, including one final read after the child exits normally. SoVITS emits it after G/D pair receipt publication; GPT emits it after the pinned epoch checkpoint callback has saved the current epoch file. Only epoch increases with matching stage, run token and generation count as progress.
- This avoids extra polling threads, full G/D rehash just for UI progress, misleading heartbeat-based epoch claims and repeated status writes when no checkpoint advanced. Sparse heartbeat includes the last checkpoint epoch only when an actual milestone receipt was observed; it does not imply a GPU batch progressed since that checkpoint. A checkpoint event is **not** optimizer resume eligibility. Faults/stale identities remain fail-closed, and no GPU processing has been run.
- Remaining work: exact source/packaging static consistency and process-stop outcomes; do not pretend progress is continuous when epochs between checkpoints are unknown.

## Device-adaptive stall observation, NOT automatic hang killing

- Added an epoch-completion event independent of full checkpoints, emitted by the source-hash-bound SoVITS/GPT trainer at epoch boundaries. It reuses the same stage supervisor loop, a separate tiny stage-scoped JSON and the canonical status writer, not another worker/monitor. Epoch evidence says only the epoch completed; no claim about checkpoint consistency or optimizer resume.
- The bounded pure `EpochStallObserver` learns a local baseline from two completed epoch intervals on the *actual user device*, then emits one suspicious-slow-progress warning after max(20 minutes, 6× slowest of the last four observed epoch intervals). With just one completed epoch, it may warn after 45 minutes of no further epoch; with zero completed epochs it emits at most one warning after 45 minutes but cannot establish a stall. If several short epochs are coalesced into one observer read, their elapsed duration is normalized by completed epoch count. Warnings never auto-kill a slower GPU or pretend to prove deadlock. Critical VRAM/RAM/disk telemetry and user Stop retain their separate fail-closed behavior.
- No new worker, no model loads, and no timer or polling loop. These thresholds are provisional source design parameters, not measured performance envelopes. Epoch/Lightning source runtime proof remains deferred.

## Recovery audit compatibility: no silent resume eligibility

- The existing developer-only `voice_lab_resume_validation.py` audit has been aligned to the current immutable Rust `TrainingRecovery` receipt: maximum 28 parts, up to 12 checkpoints per family and optional `sovits-pair-*.json` artifacts with step, G/D name, size and SHA-256 linkage to the already hash-verified archived inputs. This is a tiny metadata comparison; it never reloads checkpoint tensors automatically or rehashes large files twice.
- Both historical (`if_save_latest=True`) and new loss-resistant (`if_save_latest=False`) configuration receipts may be audited, with a blocker for legacy archives lacking a pair receipt. **The auditor never sets `resume_available=True`**. Its protected structural inspection and stricter runtime-worker acknowledgment contracts remain developer-only.
- The old release source-contract expectation for latest-saving has been updated; fixture examples are authored but unexecuted. No GPU, Rust, Python or Node validation performed.

## Safety evidence contract — current source ceiling

| Evidence | Authoritative source | Meaning | MUST NOT imply |
| --- | --- | --- | --- |
| Supervisor heartbeat | Existing run-token-bound Python status, emitted sparsely | Process supervisor can read device telemetry and run its loop | GPU batches/epochs advanced |
| Epoch activity | SoVITS and GPT pinned trainer epoch boundary, scoped tiny JSON | An epoch has completed | Full checkpoint exists |
| Checkpoint receipt | SoVITS G/D hash-bound pair or GPT saved-epoch file, scoped tiny JSON | Checkpoint file publication occurred | Tensor payload is loadable or optimizer will resume |
| Immutable recovery archive | Rust saved TrainingRecovery manifest + streamed hashes | Identical bytes/dataset provenance were preserved | Full AMP, scheduler, RNG continuity |
| Explicit developer resume audit | Developer-only `voice_lab_resume_validation.py` plus strict acknowledgment protocol | Highlights missing source/state proof and blocks invalid files | Permission to expose Resume or run training |

All source declarations above require later Windows/runtime confirmation. A hash-valid G/D receipt is not one atomic cross-file transaction. The pinned SoVITS checkpoint writer saves model and optimizer but omits full AMP GradScaler continuity and independent RNG provenance. A source-level resume pass therefore remains unavailable, including for GPUs with larger VRAM. The opt-in resume diagnostic remains developer-only and cannot be used as a production training entrypoint.

## Bounded checkpoint-pair read and worker provenance

- Fresh random stage nonce and owned process PID are required in every epoch, checkpoint and cooperative-stop acknowledgment in addition to the existing run token, generation and stage. A prior run's metadata or a different process cannot be accepted by accident; nonce/PID are correlation checks, not a security proof against local adversarial processes.
- SoVITS pair verification now scans only small JSON receipts for the epoch requested by Stop; it streams G/D hashes **only for the unique matching epoch** rather than rehashing every earlier checkpoint. File identity metadata is checked before and after each streaming hash to catch concurrent modification. Existing Rust archive copy/hash checks remain independent.
- This avoids repeated gigabyte reads while preserving fail-closed pair integrity. New regression fixtures are authored but not executed.

## Strict per-stage stop control (source-only)

- A nonexistent cooperative request remains an ordinary continue. However, an existing request with invalid JSON, symlink, wrong PID, wrong run token, wrong stage or wrong nonce now throws an explicit control error instead of being ignored. This prevents missing a stop because a control file was ambiguous, while still requiring the existing parent supervisor to reconcile nonzero exits and preserve checkpoint evidence.
- The parent similarly refuses non-dictionary or wrong-identity checkpoint-boundary acknowledgment metadata. These conditions do not promote a candidate, do not enable resume and do not add polling. Source-only fixtures authored, not executed.

## Implementation checkpoint — 2026-10-14

- **Checkpoint G/D TOCTOU:** the existing hash scan now requires before/open/after stable filesystem identity and hashes only the requested epoch's unique G/D pair. Distinct original optimizer checkpoint filenames and Rust archive-level postcopy hash verification remain unchanged.
- **Trainer-origin evidence:** epoch progress, full-checkpoint milestones and cooperative stop requests/acknowledgments carry run token, generation, stage, *owned trainer PID* and a fresh nonce created at each stage launch. This prevents accidental acceptance of files produced by older workers; it is not an adversarial OS security boundary.
- **Fail-closed controls:** missing cooperative stop request means continue; a present-but-corrupt or mismatched control record is an error, not permission to keep training. Existing supervisor owns all teardown and resource monitoring. No additional monitor or GPU allocation introduced.
- **Version scope:** the source-hash-bound adapter still matches reviewed pinned GPT-SoVITS anchors and Lightning 2.6.5 callback source in read-only inspection. This is not a Python import, model training, Windows runtime or test-suite proof.

## Stop versus completed-review serialization — 2026-10-14

- Identified a native race where `current_voice_lab_build_snapshot()` was checked before/after a file receipt but without one shared mutex covering the operation, allowing an unexpected Stop request to interleave with completion.
- Added one existing-lifecycle-mutex-backed `commit_voice_lab_review_if_active()` gate: a successful exited child with valid evaluation may create the small completion receipt and transition Evaluating → idle only while Stop cannot interleave. If Stop acquires the lifecycle lock first, this commit fails and recovery evidence is preserved without promotion. No new mutex, worker, polling, or model I/O is introduced.
- This is source-authored only and requires later Rust compilation/Windows race injection, not a proven production fix.

## Owned process tree quiescence before recovery — 2026-10-14

- The Rust completion owner now queries its own Windows Job Object active-process count (not a system-wide process scan). After the root coordinator exits, it terminates still-owned descendants and checks that no owned processes remain before reading, copying, or promoting checkpoint files. No new long-lived watcher or process is created.
- If Windows accounting or termination is unavailable/unconfirmed, the run fails closed and existing Build/Runtime checkpoint bytes are left untouched for attention. Only verified-quiescent runs may publish Review or begin immutable recovery copying. Normal root and stage child exit paths still use the original job ownership.
- Implementation is source-only; Job query layout, nested Windows jobs, driver faults and the exact error-path behavior require authorized Windows acceptance later.

## Single next step (source-only)

Review the precise interaction of trainer cooperative stop, canceled completion receipts and Rust checkpoint archive preservation under unexpected worker exits. Do not activate optimizer resume until actual SoVITS G/D, GPT optimizer, scheduler, GradScaler and RNG load/continuation can be established with independent Windows runtime evidence. Continue in `Local` without running tests, CI, compilation or CUDA processing until explicitly authorized.
