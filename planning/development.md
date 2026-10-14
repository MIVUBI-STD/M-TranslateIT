# My Voice — Active development: device-aware SAFE_CUDA

**Status:** SOURCE_CANDIDATE / NOT EXECUTED. **Branch:** `Local`. **Date:** 2026-10-14.
**Source of history:** prior commits on `Local` and `docs/knowledge/proposals/2026-10-14-my-voice-safe-8gb-training.md`. This file is the CURRENT work handoff, not an ever-growing changelog.

## Scope and owners

- The system must **discover each user's supported NVIDIA CUDA GPU** using CUDA physical UUID and matching NVIDIA sensor data, and calculate memory budget from that device's actual free VRAM. RTX 3070 8 GiB is a reference only, not a hardcoded hardware requirement. Non-CUDA training is not silently substituted.
- **Rust/Tauri** owns the Create/Improve lifecycle, authorization, selected GPU, RAM/disk preflight, run-token/generation, Windows Job Object, Stop request, completion/review receipt and immutable recovery archive.
- **One Python worker** owns stage orchestration. The existing stage supervisor measures RAM/VRAM/disk and PID-scoped training evidence. No parallel supervisor, duplicate UI poller, cloud trainer, automatic retries, or automatic optimizer resume.
- Pinned GPT-SoVITS V2ProPlus source stays immutable on disk. Source-hash-bound single-GPU adapters handle compatibility and scoped checkpoint writers. Mixed-FP16 eligibility is a provisional CUDA compute-capability policy (major >= 7), not a universal GPU support claim.

## Source-candidate safety flow

1. **Admission:** reject unknown GPU identity/VRAM, incompatible precision, or inadequate RAM/disk. GPU batch size is currently 1; allocator fractions and resource reservations are conservative but cannot guarantee no OOM.
2. **Execution:** Windows worker/stage children are requested suspended, admitted to owned kill-on-close Jobs, then resumed. Global free VRAM, physical GPU identity and system headroom are observed via the already-running stage supervisor. Stable high usage is WATCH. Sustained PRESSURE requests checkpoint-boundary stop; CRITICAL/UNKNOWN triggers owned emergency termination. The initial pressure warmup and thresholds are provisional.
3. **Observability:** sparse heartbeat means supervisor responsiveness only. Stage-PID + fresh nonce + run-token/generation-bound small epoch and checkpoint receipts record actual epoch boundaries and checkpoint file publication, respectively. Stall warnings adapt to measured per-user epoch duration and NEVER auto-kill a merely slow GPU.
4. **Stop:** manual Stop uses a create-only, run/generation-bound signal read by that SAME supervisor. SoVITS/GPT only request cooperative stop at eligible saved-epoch boundaries; other stages stop directly. A bounded timeout falls back to owned termination. No stale/corrupt control record is silently treated as permission to continue.
5. **Checkpoint:** full SoVITS G/D optimizer checkpoints use distinct atomic per-file writes; a SHA-256 pair receipt binds a common global step and epoch. GPT full checkpoints use a scoped Lightning 2.6.5 CheckpointIO adapter with atomic file publication; inference candidates retain their pinned format. Four SoVITS pair-save epochs + five GPT full-save epochs bound I/O and fit the recovery count limit, avoiding redundant `last.ckpt` copies.
6. **Completion and recovery:** one Rust lifecycle mutex serializes **Stop versus completion receipt + Evaluating → idle**. Upon Python root exit, Rust checks the owned Windows Job's active-process count and refuses Review/recovery if descendant quiescence cannot be verified. Interrupted checkpoints are archived only after this gate, with free-disk admission and independent copy/destination hashes. Reuse source part hashes for G/D pair metadata instead of rereading GB files. Existing selected Meeting voice/recordings are not replaced on failure.
7. **Resume:** preserved checkpoints remain candidates for **developer-only integrity diagnostics**. A correct hash or saved optimizer file does not prove restored optimizer, scheduler, GradScaler, RNG, or training continuity. `resume_available` remains **false**. Any restored-training implementation must be separately validated and explicitly approved.

## Important limitations (do not claim completed product)

- No Rust/Node/Python suite, cargo check, build, Windows Job API run, GPU training, quality listening, installer inspection, simulated OOM, or hardware verification was performed as part of the source-only development. Source matches and authored test cases are NOT test passes.
- Device selection/allocator reservations are provisional. CUDA OOM and driver faults can occur between readings. Windows nested Job compatibility, thread suspension/release, graceful Stop, exact Lightning callback interactions and checkpoint saves must be tested on supported devices when the user authorizes validation.
- A G/D pair is two separately committed files, NOT a single atomic transaction. Checkpoint staging + pair hashes limit partial publication but cannot prove optimizer resume. The recovery archive may conservatively require manual attention when source files are incomplete.
- The stage PID and nonce are **correlation**, not a security boundary against intentionally malicious local processes.

## Next focused source-only work

**P0:** Review any remaining cancellation race between Rust completion, checkpoint preservation, and a late manual Stop without adding locks or workers.

**P1:** Verify the Windows Job accounting struct/API assumptions and pinned Lightning 2.6.5 source changes against the reviewed dependency revisions. Author bounded source regression cases for failures, not GPU jobs.

**P2:** Make recovery attention/diagnostic messages precise when owned descendant termination or checkpoint write cannot be verified; preserve the original work tree.

**STOP boundary:** Do not run tests, cargo/Node/Python execution, Windows/CUDA probes or end-to-end training until explicitly authorized. Do not enable Resume. Do not alter `main`. Work remains in `Local`.
