---
id: document.knowledge.proposals.my-voice-safe-8gb-training
class: DOCUMENT
domain: knowledge
role: GUIDE
authority: REFERENCE
lifecycle: ACTIVE
---

# My Voice — Safe 8 GB Training and Failure Containment (design proposal)

**Status:** DESIGN ONLY / NOT IMPLEMENTED / NOT VALIDATED. Do not treat this RFC as a shipped safety control.
**Baseline:** MIVUBI-STD/M-TranslateIT `Local` at `f4389b05ae9607659f8c3498335ef10b34b7c3e6` (2026-10-14).
**Target:** single NVIDIA GeForce RTX 3070 8 GB GDDR6 on Windows, GPT-SoVITS V2ProPlus with existing pinned model assets and one Rust/Tauri control authority. 12 GB is OPTIONAL headroom, not a hard eligibility requirement.
**User instruction:** DESIGN FIRST; do not run tests, host CI, local GPU experiments, audio validation, or release/install validation during this design phase.
**Priority:** protect accepted recordings, immutable snapshots, checkpoint evidence, current Meeting voice, host availability and diagnostics. A GPU OOM/native access violation may still occur; containment and fail-closed behavior, not zero-crash guarantees, are the objective.

## 1. Source-visible risks (why a detection badge is insufficient)

| Finding at baseline | Exact source area | Required design response |
| --- | --- | --- |
| `batch_and_half()` uses CUDA **total** VRAM plus 0.4 GiB and divides by 2; on 8 GB it can choose batch 4, without accounting for actual free VRAM, peak sequence length, other apps or differing SoVITS/GPT needs | `voice_lab_gpt_sovits_build.py:275-347` | Replace by per-stage bounded memory admission + conservative single-example profile; never calculate batch from total VRAM alone |
| GPU index `0` and 16-bit CUDA precision are assumed in training config; the preparation stage hardcodes `is_half=True` regardless of hardware inspection | `voice_lab_gpt_sovits_build.py:228-269,294-347` | Bind selected physical GPU identity through every child, explicitly decide per-stage precision, and prohibit surprise device fallback |
| Child launch checks exit code only; `subprocess.run` has no stage watchdog, semantic CUDA OOM classification or staged memory policy | `voice_lab_gpt_sovits_build.py:198-212` | Bounded cancellable stage supervisor, heartbeat, memory telemetry, typed failures and complete descendant reaping |
| Pinned SoVITS trainer uses `mp.spawn`, Gloo on Windows and DDP wrapping on CUDA including single GPU. Pinned GPT Lightning trainer also selects DDPStrategy on CUDA | `GPT_SoVITS/s2_train.py`, `s1_train.py` at source revision `d523079fc05d9a8028d6085bffe4a2757c32abb6` | Treat one-GPU Windows DDP/native-crash risk separately from VRAM OOM; a pinned/approved single-GPU trainer compatibility change is an explicit scoped decision, NOT a silent hotpatch |
| Evaluation holds GPU TTS model per candidate; `gc.collect()` and `torch.cuda.empty_cache()` cannot guarantee all CUDA/process allocations return | `voice_lab_gpt_sovits_build.py:536-625` | Spawn evaluation candidates in isolated, sequential child processes with independent admission and verified teardown |
| Rust maintains resource ownership, initial frozen dataset, checkpoint preservation and failure events, but child status has no run-specific token and cancellation may force-kill the Python tree | `voice_lab_build.rs`, `voice_lab_build/phase.rs`, `voice_lab/storage/training_recovery.rs` | Preserve the existing sole Rust owner; add generation-bound status receipts, graceful bounded stop with hard tree cleanup, and quarantine incomplete checkpoints |
| Current compute badge reports CUDA name and total VRAM, but neither **free VRAM** nor independent backend build admission | `my_voice_compute_probe.py`; `MyVoiceBuild.svelte` | Keep the badge advisory; enforce admission natively at every Create/Improve entrypoint and each stage, including direct backend invocation |

A public report describes a Windows single-GPU DDP access violation on a 12 GB RTX 4080 Laptop GPU, not necessarily the exact same pinned revision: https://github.com/RVC-Boss/GPT-SoVITS/issues/2806. This is a plausible failure mode, NOT proof that this user's 3070 will crash. Do not misclassify every native crash as VRAM OOM.

## 2. Core invariants (must never be weakened)

1. **A1 — Existing work remains intact:** accepted WAVs, canonical/native-rate partners, frozen source snapshots, backed-up training recovery evidence and approved actor versions are immutable. Candidate work always happens in a fresh per-run disposable directory.
2. **A2 — No premature actor promotion:** only a fully closed, structurally validated and content-hashed candidate, all existing eight held-out samples and explicit user review may enter the existing atomic promotion path.
3. **A3 — No unverifiable continuation:** recovering a file is not optimizer resume; interrupted SoVITS/GPT checkpoint files remain archived as evidence, never automatically used for training continuation.
4. **A4 — No silent degradation:** do not change device to CPU, output language, model family, precision, training dataset, record duration or quality thresholds to conceal VRAM failure.
5. **A5 — Backend authority:** Rust rejects build/admission even if a caller bypasses Svelte. The GUI never decides memory safety on its own.
6. **A6 — No overlap:** Meeting, Quick Preview, inference and My Voice GPU-heavy build/evaluation are mutually exclusive under existing application resource authority. An external GPU consumer can still compete; respond to that pressure explicitly.
7. **A7 — UNKNOWN is not PASS:** absent telemetry, lost worker heartbeats, invalid run tokens, failed disk probes or uncertain process termination fail closed; no auto-enable of Create.
8. **A8 — Real hardware limits:** PyTorch allocator caps are **not** absolute GPU caps. Driver, non-PyTorch kernels, child processes, OS display and other apps consume additional VRAM. Zero OOM/crash cannot be promised.

## 3. Default policy: SAFE_8GB (initial design values, NOT empirically proven)

- Single selected physical GPU only, bound by stable adapter identity / CUDA-visible index mapping; reject any unexpected change between admission and stage creation.
- SoVITS physical `batch_size=1`; GPT physical `batch_size=1`. Do not start at batch 4 merely because VRAM totals 8 GB. Other GPU sizes can have separate profile versions later.
- Prefer CUDA mixed 16-bit where the *pinned* training code already supports it; report true precision. Preserve loss-scaling and numerical checks. Do not assert this is stable without later runtime evidence.
- Independent memory constraints for **each stage** (feature extraction, SoVITS, GPT, candidate evaluation); never reuse a prior stage's budget or assume an identical peak.
- GPU memory budget, initial design: let `T=physical_total_bytes`, `F=live_free_bytes` and `R=max(1.5 GiB, 0.20*T)`. Cap planned torch allocation at `B=min(0.70*T, max(0,F-R))`. For an 8 GiB card, the static 70% cap is ~5.6 GiB, but the free-minus-reserve term can reduce it further. The numeric reserve is PROVISIONAL, must be revisited after an explicitly authorized native benchmark; it is neither a benchmark result nor a certified capability guarantee.
- If telemetry is unavailable, `F<=R`, driver is unstable, device identity is ambiguous, GPU is occupied by another TranslateIT task, or the chosen stage cannot be admitted under budget, return `INSUFFICIENT_SAFE_GPU_HEADROOM` / `GPU_ADMISSION_UNKNOWN`, never start "optimistically."
- A large free number does not establish a known stage's peak need; maintain versioned observed peak-watermarks **only after real successful future runs**, with same engine/config/device identifiers. Before such observations, low-memory profile remains an experiment, not a guarantee.
- Require a **nonzero driver/OS reserve after allocation**. Observe actual global free VRAM continually; if it falls below the configured soft margin, request stop at a safe batch boundary if available. An OOM allocation may occur earlier; catch and contain it anyway.
- PyTorch `set_per_process_memory_fraction` may provide a secondary allocator guard **inside actual CUDA-using worker children**, not only the Rust supervisor or Python parent. It bounds PyTorch caching allocator requests only; do not claim it limits VRAM globally. Handle `torch.cuda.OutOfMemoryError` as terminal for that attempt. Any subprocess that cannot apply/confirm the cap must be rejected in a strict safety profile.
- Avoid advanced switches (gradient checkpointing, CPU offload, gradient accumulation, workers/pin-memory overrides or independent FP16 patches) until exact pinned trainers are inspected and their semantics defined. Do not silently lower quality or edit the immutable recording dataset.

**Practical RTX 3070 interpretation:** 8 GB is a first-class design target; 12 GB is not required. If the actual SoVITS/GPT stage will not fit `batch=1` within a safe 8 GB budget, STOP with a descriptive hardware/resource limitation. The model should not be made "successful" by removing safety margins or secretly using a different engine.

## 4. Native preflight (authoritative, shortly before launch)

The existing explicit hardware badge becomes informative only. Rust must own a **new** bounded compute-admission result for BOTH Create and Improve, derived from actual selected worker Python and the correct CUDA device. Avoid a second long-lived GPU worker or second state manager.

Preflight inputs: pinned source/model manifest health; selected dataset ID and take count; canonical audio lengths and max-duration distribution; GPU total/free VRAM and UUID/index; application Meeting/Preview/build lock; GPU training-profile ID; available disk and RAM/pagefile; presence of required source and model assets.

Outputs:
- `ADMITTED`: provisional safety budget, frozen config digest, GPU identity, run token and TTL-bound snapshot.
- `BLOCKED_EXTERNAL_GPU_PRESSURE`, `BLOCKED_GPU_HEADROOM`, `BLOCKED_DISK_SPACE`, `BLOCKED_UNSUPPORTED_DEVICE`, `BLOCKED_TRAINER_COMPATIBILITY`, or `UNKNOWN` with a user-readable reason.
- Admission must be **rechecked immediately in the process that performs each CUDA stage**. A good result a minute ago is not sufficient.
- Disk budget covers disposable preprocessing, candidate WAVs, checkpoint staging, immutable recovery-copy headroom and rollback. No automatic deletion of protected history when disk space runs low.
- CPU-only route is not a supported safety fallback for GPU failure. The existing informed CPU UI path remains labeled UNVERIFIED until a separate supported execution contract is established.

## 5. Process and memory containment

```text
Tauri My Voice UI — read-only presentation and explicit user actions
    |
Rust single resource owner (create/improve admission, run ID, lifecycle)
    |-- OS disk/RAM/GPU global telemetry; budget and alarms
    |-- owned Windows Job Object / process-tree termination authority
    |-- immutable input snapshot + per-run staging
    |
Python voice_lab_build.py — one build coordinator, no meeting inference
    |-- preprocess stage child + admitted CUDA cap
    |-- SoVITS single-GPU stage child + admitted CUDA cap
    |-- GPT stage child + admitted CUDA cap
    |-- candidate 1 evaluation child (then exit)
    |-- candidate 2 evaluation child (then exit)
    |-- candidate 3 evaluation child (then exit)
    |
Integrity-bound candidate -> eight held-out previews -> human approval
```

- A real Windows Job Object with kill-on-close ownership is the intended process-tree guard; current `taskkill /T /F` is a best-effort fallback, not a proof that all spawned descendants relinquished VRAM. Do not terminate unrelated Python/GPU processes.
- Model initialization, first training batch, checkpoint writing and evaluation synthesis must be treated as separate high-peak moments. Monitor global GPU free, per-process usage if available, Torch allocated/reserved/peak, RAM and available disk. NVML/nvidia-smi values are telemetry, not exclusive memory reservation.
- Heartbeats and memory updates must be bounded and provenance-bound to one run ID, stage attempt, process identity, time and monotonically increasing sequence. Distinguish "busy inside known slow stage" from "worker lost/unknown." Never assume wall-clock timeout alone proves a healthy worker failed.
- The supervisor may request a **graceful stop** before the next safe boundary. After a documented grace deadline, terminate the owned process tree. If the worker was saving a checkpoint, its unfinished output is **never** accepted as a valid checkpoint.
- End of a subprocess is the strongest VRAM-release boundary. Calling `torch.cuda.empty_cache()` may release unoccupied cached allocations but is not a full process or driver reset. Do not use this function as the only safety mechanism.
- The current single-GPU SoVITS DDP route requires a separate compatibility decision. Preferred direction: explicitly versioned, source-hash-checked single-device adapter removing unnecessary one-GPU DDP/Gloo/spawn while preserving pretrained weights, checkpoint formats and state-loading behavior. **Do not silently modify pinned GPT-SoVITS source or claim behavior is fixed until formally reviewed.** Until resolved, the admission state may be `BLOCKED_TRAINER_COMPATIBILITY`.

## 6. Deterministic lifecycle, storage and restart

Proposed run states:
`requested -> admitted -> dataset_frozen -> preparing -> training_sovits -> training_gpt -> evaluating_candidate_N -> verifying_candidate -> review_ready`.

Failure states:
`resource_blocked`, `stopping`, `stopped`, `gpu_oom`, `gpu_device_lost`, `native_worker_crash`, `worker_timeout_unknown`, `disk_exhausted`, `output_integrity_failed`, `recovery_needs_attention`.

- Persist immutable run descriptors (run ID, dataset snapshot SHA, revision, config digest, GPU identity, attempt). Status receipts must carry run ID/generation, stage, sequence and exact phase identity so an old `status.json` cannot promote a new run.
- All phase output goes to per-run `.pending` paths. A phase is committed only after expected bounded assets are present, structurally valid, hash-verified and atomically made visible. An upstream checkpoint writer that cannot guarantee atomic writes needs a guarded staging adapter; a partial file is quarantined, not selected.
- NEVER clean a previous workspace until any checkpoint candidates have been preserved and SHA-verified in the existing immutable `TrainingRecovery` owner. If preservation is uncertain, block the new run without altering old evidence.
- Candidate cannot become `review_ready` from an exit code, file existence, or JSON status alone. Preserve existing held-out provenance plus exact candidate identity, hashes and explicit listen/approve gate.
- Interrupted attempts are not optimizer resumes. A future user-approved "Try again" may start a **new attempt from pinned pretrained weights on the same frozen dataset**, only after prior evidence has been protected and the resource issue is addressed. No silent rerun loops.
- If no safe configuration fits 8 GB (`batch=1`, capped allocation, clean exclusivity), stop and explain why. Do not secretly change the dataset or introduce cloud training.

## 7. Failure response matrix

| Failure | Safe response | Explicitly forbidden |
| --- | --- | --- |
| External app consumes VRAM before start | Block admission, show available vs required reserve, allow manual recheck | Starting with stale total-VRAM-only estimate |
| Memory pressure during known stage | Request stop at a safe boundary; guard with allocator limit; if exceeded, report `gpu_oom`; terminate owned children after grace | Blindly retrying with same batch or freeing other applications' VRAM |
| Native child exits `0xC0000005` or CUDA device lost | Record `native_worker_crash` / `gpu_device_lost` (not necessarily OOM); verify full descendant exit; quarantine partials | Calling it out-of-memory without evidence or continuing the pipeline |
| App or worker is killed mid-save | Recover immutable dataset, audit partial checkpoints and preservation archive; block new run on uncertain recovery | Promoting truncated `.pth/.ckpt` or clearing evidence |
| Disk becomes full | Stop before new output when possible, keep protected originals, report required free space | Automatic pruning of accepted WAVs or approved voice versions |
| One held-out evaluation child fails | Retain earlier candidate evidence; terminate remaining work safely; mark candidate evaluation incomplete | Declaring prior candidate or voice usable without full evaluation |
| UI closed, request reply lost | Reconcile native run ID/status; preserve worker ownership, no duplicate start | Assuming idle and starting a second build |
| End-to-end run succeeds but voice sounds poor | Keep candidate unselected until user listens and approves | Auto-selecting based on speed, similarity score or exit code |

## 8. UX design for an 8 GB GPU

Display a compact panel before Create **and** Improve:
- **Detected GPU:** NVIDIA GeForce RTX 3070 / total 8 GB / current free VRAM read separately.
- **Training profile:** `SAFE_8GB` — batch 1, one stage at a time, no Meeting/Preview overlap.
- **Preflight:** `Not checked`, `Checking`, `Safe to attempt (not guaranteed)`, `Insufficient free VRAM`, `Trainer compatibility requires attention`, `Unknown`.
- **During training:** current actual stage, GPU free memory, stage peak allocated/reserved when available, and a plain-language action such as "Stop safely". Do not show fake percent/ETA.
- **After failure:** "Your saved recordings and previously approved Meeting voice were not intentionally replaced"; show separate "Recovery verification needed" if checkpoint integrity is unknown. Avoid unconditional data-safety claims without storage evidence.
- No user-facing configurable GPU percentage until per-GPU policies and bounds are established. The safe profile is an internal versioned policy, not an arbitrary slider.
- On safe stop, show a real resource release acknowledgment or "Still stopping"; never say completed while subprocess/descendants remain.

## 9. Implementation responsibility and sequencing — NOT STARTED

**P0 — Native admission first.** Add Rust-owned single admission gate for every Create/Improve entry (including direct invokes), selected GPU identity, free-vs-total status, disk budget and resource conflict. Keep MyVoice Svelte presentation-only and preserve current Meeting locks.

**P1 — Memory-capped stage runner.** Preserve one Python/GPT-SoVITS codebase; add child-internal CUDA cap and memory/heartbeat reporting with run-scoped tokens. Start SoVITS/GPT from physical batch=1 under `SAFE_8GB`. Align CUDA precision and GPU identity through all feature/evaluation stages. Reject unknown caps rather than silently running unrestricted.

**P2 — Single-GPU Windows compatibility and owned process tree.** Review the exact pinned `s2_train.py` / `s1_train.py` DDP behavior; make an explicit separately approved, checksum-bound compatibility change or retain a truthful compatibility block. Add Windows Job Object ownership + graceful-stop/kill escalation, descendant exit acknowledgment.

**P3 — Evidence and recovery.** Per-stage atomic status/receipts, staged checkpoint publication, run-specific immutable manifest, exact byte/hash validation, interrupted checkpoint preservation and startup reconciliation; no automatic optimizer resume.

**P4 — Evaluation isolation + unified UX.** Evaluate candidate checkpoints in bounded sequential child processes, preserve original score/held-out/approval contracts, surface typed failures and actual stage telemetry without inventing progress.

Do not implement half of the native gate such that only the UI protects it; do not patch the trainer without a reviewed owner/contract change. Keep the existing `Local` source candidate and all retained audio, model and backup paths unchanged during design.

## 10. Future acceptance requirements (explicitly deferred by user)

When the user later authorizes validation, require traceable end-to-end evidence for RTX 3070 8 GB, foreign GPU pressure, long accepted utterances, SoVITS/GPT first batch, all evaluation candidates, CUDA OOM, native child crash, cancel during save, app termination/restart, full protected checkpoint archive and unchanged selected voice; compare exact pinned source/config. Until such evidence exists, the result is **DESIGNED / UNVALIDATED**, never "8 GB training certified".

## 11. External documentation used for rationale

- NVIDIA RTX 3070: https://www.nvidia.com/id-id/geforce/graphics-cards/30-series/rtx-3070-3070ti/
- PyTorch CUDA device free/total memory and allocated/reserved metrics: https://docs.pytorch.org/docs/stable/cuda
- PyTorch allocator fraction applies to the per-process caching allocator, not an absolute global VRAM reservation: https://docs.pytorch.org/docs/main/generated/torch.cuda.memory.set_per_process_memory_fraction.html
- Public upstream single-GPU Windows crash report: https://github.com/RVC-Boss/GPT-SoVITS/issues/2806
