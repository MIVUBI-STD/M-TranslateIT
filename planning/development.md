# TranslateIT — staged source repair after end-to-end audit

## Current Status

The Windows source CI on commit `14eaca42cd54eafacfd045e941bc26be7c43908e` completed successfully. This establishes frontend, Python, and Rust/Tauri source/compile/unit checks for that revision, not installed Windows acceptance. Any subsequent source or workflow change needs its own matching CI proof. The GitHub default branch `main` remains reserved for explicitly approved final releases.

## Active Boundary

The owner-approved single hosted Windows source CI is permitted; it cannot exercise CUDA hardware, private models, live microphone, VB-CABLE, native audio, or installed application behavior. No per-change tests on the user's PC: all native checks belong in one planned, integrated Windows session after source admission.

## Next Step

Close pre-local source evidence gaps on a single `Local` SHA: check WorkerRuntime lock consistency, production npm security advisories, ASR/translation/TTS quality-evaluator contracts, and documentation/source flow parity. Require a completed successful aggregate Windows CI on the resulting SHA. Then prepare one integrated test-ready candidate for developer-mode Windows testing: `DEV.cmd doctor` → locked `setup` → `check`/`build` → app startup → real GPU/models/audio/Meeting acceptance. Installed Setup/signing/release proof remains a separate conditional gate; do not ask for incremental user-PC tests.

**Active:** Owner-approved automatic/manual Windows hosted **source validation** preceding the first integrated local Windows acceptance. **Branch:** `Local`. The existing successful compile/test evidence is revision-bound; GPU, installed app, VB-CABLE, native audio and actual Meeting reception remain `UNKNOWN / NOT EXECUTED`.

**Active source candidate:** Persistent local Meeting transcript archives remain available across Stop, restart and HTML export until explicit user Delete. A versioned finalization receipt records the saved entry count; the Saved Transcripts label now explicitly distinguishes finalization **count verification** from content verification. Orphaned `.tmp` files, count mismatches and legacy markers remain unverified rather than silently accepted or deleted.

**Current source change:** The finalized-session HTML export now checks the actual background writer owner **after** acquiring the saved-session operation guard. Runtime session authority may already have been revoked during Stop while archive writes are still draining; a live writer for the target session now blocks final export with a Retry Stop message. Once the writer is truly gone, interrupted/unverified archives remain exportable with the existing explicit completeness warning, and no source files are changed by export.

**Interrupted export privacy:** A crash can leave user speech in app-created `Transcript_<timestamp>_<nonce>.html.partial` files even though no successful HTML receipt exists. On a *subsequent explicit Export HTML*, the app now scans only that session's export folder and removes only strictly matching app-generated, unpublished partial filenames. Completed `.html` files and the saved source archive are never removed by this cleanup. If cleanup of a failed export's own temp file fails, the error now warns that unfinished HTML may remain rather than silently claiming cleanup. This does not run background retention cleanup.

**Prior bounded-read work:** Full HTML uses one directory inventory for paths/count/completeness and validates each JSON once into the temp HTML. Read Preview is limited to 240 entries/4 MiB encoded JSON; HTML Preview to 12 entries/512 KiB, independently of the full export. The archive retains bounded writer resources and user-controlled deletion; summarization remains canceled.

**Known caveat:** Stop can finalize only the bounded in-memory turns. Much older already-evicted archive entries can still hold `pending` in raw JSON; the existing saved reader/export normalizes their presentation to unavailable after Stop. Queue or disk admission failures are counted/warned, and a complete-entry-count receipt cannot prove every ASR utterance was captured. No claim of unlimited durability or remote meeting delivery. No AI/audio pipeline changes.

**Privacy/product scope:** Local Saved Transcripts remain available until explicit user Delete, even if HTML is exported. In-progress HTML preview distinguishes pending from failed; after Stop only recognized, saved entries are exported. The archive uses a single source-validation reader, bounded producer queue and versioned entry-count completion receipt with best-effort error warnings; incomplete/corrupt records are not silently portrayed as complete. Summarization stays canceled.

**Efficiency and remaining boundaries:** Live UI shows up to 240 entries; its memory buffer keeps up to 4,096/8 MiB while the separate archive remains disk-backed. The queue is bounded by 256 records and 128 MiB estimated pending bytes; JSON persistence is streamed. A session may still lose entries on queue overflow, disk faults or crash, with best-effort warning. A matching receipt count does not prove content integrity, and a valid edited JSON entry cannot be detected as tampered without a stronger cryptographic receipt. Large-session directory listing scales with file count, and there is no audited upper disk quota or browser HTML limit. Summarization stays canceled.

**Save As and proof boundary:** Native Save As is NOT implemented; the source dependency lock does not contain the Tauri dialog plugin. App-owned export + Open File/Folder/Copy Location remain the only functional path in source. Rust compilation, Svelte typecheck, executed tests, crash fault injection, browser print and TARGET_WINDOWS acceptance are `UNKNOWN / NOT EXECUTED`.

**Contract correction:** PR-100/101/103/104, D10/D11, Windows runbook and product/storage entrypoints now agree on the Meeting-specific recognized-text archive: automatic local session retention until explicit Delete, temporary raw audio and separate manual HTML export. Static patches since that alignment additionally tightened writer Stop/Retry Stop ownership, archive listing/error truth, cross-session caption isolation and UI Start/Stop action fencing; none has executed compiler or Windows proof.

**Current cross-subsystem source observation:** Text Swap previously could overwrite source/result edits made during async settings persistence; edit revisions now preserve the later drafts. My Voice recording/build/selection use read-only verification and status retry on unknown bridge responses. Settings distinguishes confirmed device persistence from failed display refresh. Source-only review also closed the no-snapshot Meeting cleanup gap: ApplicationRuntime audio lock, Settings native persistence, Built-in/Saved Voice selection and build, Mic Test Start and Guided Recording Start now reuse the canonical Retry Stop admission fact. D3/U2 and A9/U3 require real native fault cases; none has executed yet.

**Admission boundary:** Source and test contracts for the currently scoped Meeting, Text, My Voice and Settings workflows were inspected at the `Local` head following these patches. No new parallel subsystem owner, cloud CI, engine swap or dependency was introduced. Available pinned lockfiles/toolchain definitions are not compiler/model evidence. Known unproven areas include >60-second uninterrupted-speech loss/segmentation, disk/permission faults, actor training/voice quality, GPU pressure, actual virtual-mic reception and Windows layout. Native Save As is not in the developer-mode baseline. Preserve these limitations as explicit native acceptance questions; never silently promote them to PASS.

## Retained My Voice source candidate (paused)

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

## Generation handoff remains busy until owned process release — 2026-10-14

- Fixed a post-completion race: the small Review receipt is atomically published while the previous Python process may still be owned by the completion thread. Start/Approve/Meeting-change now treat that existing process ownership as busy until its PID/Job and generation are released.
- The canonical My Voice status defers `evaluation_ready` and `can_build` during this short finalization window, exposing a presentation-only `finalizing` status that disables repeated Stop requests while retaining the existing status polling. The underlying lifecycle remains unchanged. No new state field, worker, mutex, or polling loop is introduced.
- Source only. The window and UI behavior must be confirmed during eventual Windows acceptance.

## Source hardening: Windows accounting failure paths — 2026-10-14

- Cross-checked the owned Windows Job's `JOBOBJECT_BASIC_ACCOUNTING_INFORMATION` ABI against Microsoft's published structure, and authored a Windows-gated 48-byte compile-time layout assertion.
- When querying the active descendants fails, Rust now **attempts owned-Job termination but still returns the original accounting error**. It does not confuse a failed query with a stopped tree and does not start Review or checkpoint archiving. `KILL_ON_JOB_CLOSE` remains a fallback.
- Windows supports nested Job trees on current supported versions, but nested assignment/runtime compatibility is still unverified. No new process, background monitor, polling, or arbitrary external process termination was added.
- Changes are source-only; Windows compilation and Job API fault-injection remain disallowed until authorized.

## Latest optimizer candidate selection (source-only) — 2026-10-14

- Developer-only recovery audit previously used lexicographic labels (e.g. `gpt-10.ckpt` before `gpt-9.ckpt`) to select the last checkpoint for explicit tensor inspection. The latest candidate is now selected from the **original saved epoch and global step**, and from the original SoVITS G/D global step, not archive label order.
- Unrecognized historical checkpoint names (for example `last.ckpt` without trustworthy source epoch) block optional tensor inspection instead of silently selecting an arbitrary file. All byte/hash verification remains intact and `resume_available` remains **false**.
- Regression fixtures are authored but unexecuted, with no torch import or checkpoint tensor reads performed during this change.

## Windows Job handle lifetime before quiescence — 2026-10-14

- After `Child::try_wait`/`wait`, the Rust completion owner now explicitly drops the reaped `Child` handle **before** asking its owned Job for the active-process count. Windows process accounting can retain process-object references until handles are released; retaining the root handle through the query could falsely block legitimate completion/recovery.
- Descendant cleanup and fail-closed behavior are unchanged. No extra monitor, duplicate wait, or global process kill is added. Windows handle semantics remain to be confirmed during authorized runtime testing.

## CPython subprocess HANDLE lifetime before stage Job accounting — 2026-10-14

- Pinned CPython 3.12 `subprocess.Popen` retains a Windows process `Handle` object until it is closed/finalized. After the stage root has exited and its returncode is cached, the existing Python supervisor now explicitly closes that root HANDLE before querying its owned Job's active-process count. This prevents a retained Python handle from potentially causing a false `stage_descendants_still_running` failure on otherwise successful stages.
- The Job remains owned by the same supervisor and still kills true descendants if any remain; a missing/invalid HANDLE refuses to infer quiescence. No new subprocess, worker, polling loop or GPU allocation is introduced.
- Source-only, without Python execution on Windows. This relies on the currently pinned CPython 3.12 `Popen._handle` implementation, which should be reconfirmed if the Python runtime changes.

## Hard-stop timeout and cleanup attention gate — 2026-10-14

- Fixed an unbounded `Child::wait()` after forced cancellation / failed `try_wait`: the existing completion thread now makes only a bounded eight-second reaping attempt and **always classifies forced Stop as incomplete**, irrespective of the eventual child exit code.
- If Windows Job quiescence is unverifiable, the finalization path **keeps the old generation/Job handle in its existing process owner**, blocks new Start/Approve/Meeting changes, shows an explicit attention message, and refuses to clear the original Runtime work tree. It does not claim successful cancellation or archive integrity.
- No additional OS watcher, lock, process or progress poll was introduced. These code paths are still not runtime-tested and must remain fail-closed until Windows fault-injection is authorized.

## Unexecuted Windows failure acceptance matrix

These are future **acceptance scenarios**, not tests performed or runtime results. Test only after the user authorizes validation. In every scenario the existing Meeting voice, original recordings and immutable approved versions must remain unchanged.

| Scenario | Expected safe result | Existing source owner |
| --- | --- | --- |
| Stop during dataset preparation or before Python spawn | Do not launch GPU work; leave saved recordings untouched | Rust build lifecycle |
| Stop during SoVITS/GPT training | Request checkpoint-boundary stop using the existing supervisor; honor bounded deadline and never claim success | Python stage supervisor + Rust process owner |
| Trainer ignores Stop and Job termination succeeds | Forced stop classified **incomplete** regardless of eventual root exit code; preserve verified checkpoint candidates | Rust completion owner |
| TerminateJobObject fails or root cannot be reaped promptly | No unbounded wait; refuse Review; query owned Job quiescence before recovery | Rust Job owner |
| Job active-process accounting unavailable or descendants remain | Attempt owned termination, preserve existing runtime files, retain ownership gate, prevent new Create/Approve/Meeting changes | Rust Job/accounting + process store |
| Python stage exits while child process is still present | Reject successful stage; owned subprocess tree must stop before any next stage | Python stage supervisor |
| New SoVITS G/D or GPT checkpoint is partially written | Reject partial/pending artifacts; keep older checkpoint and original training work for attention | Atomic writer + Rust recovery |
| Checkpoint source changes while being copied | Reject archive, preserve original Runtime work; no optimizer resume | Rust immutable recovery |
| Stop races with completed evaluation | One existing lifecycle mutex decides Stop or Review; never publish a cancelled result | Rust lifecycle receipt gate |
| App closes abruptly while Job children exist | Owned kill-on-close must be verified on target Windows; no promise until tested | Windows Job Object |

**No speculative retry, auto-resume, GPU scaling or background monitors.** The existing active-process owner and canonical status remain the only runtime authorities.

## Windows suspended process admission cleanup — source-only

- A suspended child not yet admitted to a Job **cannot** be killed by terminating that empty Job. Both the Python stage supervisor and Rust top-level owner now attempt to terminate the **exact created process handle** on Job assignment failure and use a bounded reaping period. No PID enumeration, unrelated task kill, or extra worker is used.
- On the rare failure to prove that the suspended Rust process stopped, the native lifecycle stays in cancelling/attention state and prevents launching another training generation rather than claiming a clean failure. These exceptional paths cannot guarantee orphan prevention when Windows itself refuses termination; explicit diagnostics remain required.
- This is unexecuted source; do not claim fault containment is validated.

## Source-only POSIX process group ownership — 2026-10-14

- The stage supervisor no longer treats root PID exit as proof that all POSIX descendants exited. Because a POSIX stage starts in its own new session, the supervisor checks its specific stage process-group ID even after root exit and sends SIGKILL only to that dedicated group if descendants remain.
- Uncertain group ownership, permissions, and leftover descendants continue to fail closed. This **does not** enable My Voice training on Linux/macOS: the current native admission is Windows-only, and no cross-platform training claims are made. The main Rust non-Windows process owner needs separate hardening before any future non-Windows backend is supported.
- No new stage, process, watcher or polling channel was introduced. Source fixtures authored but not executed.

## Accurate attention status after unconfirmed cleanup — 2026-10-14

- When Rust cannot prove owned process cleanup, it retains the active cancellation gate and an explicit checkpoint/work preservation message. Both native My Voice status and the existing Svelte UI now **display this attention message**, rather than indefinitely claiming “Stopping safely”.
- The runtime event is emitted as a failed build when recovery/quiescence needs attention, even if Stop had been requested. This changes diagnostics only; it never claims Review or resume completed.
- No additional state owner, worker, polling or I/O channel. Source-only; not runtime tested.

## Repair of stale source regression contracts

- Previously authored Node source assertions referenced an earlier Rust expression (`!cancelling && exit...` and `let recovery = if ready`) that no longer matches the hardened lifecycle. Contracts now check the actual `owned_tree_quiescent`, `cancelling_before_review` and `recovery_failed` gates.
- This removes known future false-negative source failures without weakening the actual safety conditions. Tests were authored only, not executed, in accordance with the user's source-only boundary.

## POSIX subprocess group cleanup audit — source-only (2026-10-14)

- The Python stage supervisor already checks for surviving same-process-group children
  when the root exits and rejects stage success when they remain. A second issue
  existed during exceptional termination: after sending SIGKILL to the isolated
  process group, the cleanup path waited only for the root process.
- Cleanup now performs a bounded, post-root-wait group-existence check. If the
  original stage process group does not disappear, it raises an explicit
  `posix_group_cleanup_not_confirmed` failure instead of inferring cleanup
  from the root's return code. Regression cases use mocked process ownership;
  no process spawn, real signal, Python test suite, or native execution was run.
- POSIX process-group ownership does **not** track descendants that deliberately
  move to a separate session/group. It is not a general process-tree guarantee.
  This does not enable Linux/macOS My Voice training, change GPU admission,
  alter checkpoint publication, or imply Windows Job behavior was tested.

## Rust top-level non-Windows ownership audit — source-only (2026-10-14)

- Native Create and Improve share an early Windows-only admission guard before
  the dataset, accepted recording selection, GPU checks or child spawn. An
  available CUDA probe on Linux does not permit My Voice training.
- The previously unconditional non-Windows `owned_tree_quiescent = Ok(())`
  was unsafe: root exit and `kill -TERM <pid>` do not prove descendant
  cleanup. Its defensive fallback now remains `Err`, preserving Work files,
  preventing review publication and blocking another build rather than
  promoting unsupported cleanup to success. Any fallback attention text
  is explicitly non-Windows, not a claim that a Windows Job Object exists.
- The existing Rust source-contract suite now checks both entrypoints and the
  fail-closed completion/recovery gate. These checks were authored only;
  Node/Cargo, process execution and target-Windows validation were NOT RUN.
  No cross-platform training support or POSIX top-level process-group owner
  was introduced; Windows Job implementation remains unchanged.

## Windows My Voice process ownership and checkpoint finalization audit — source-only (2026-10-14)

- Found a definite exceptional-path hang: after Python spawn, a poisoned
  `process_store()` mutex could execute unbounded `child.wait()` despite
  the existing forced-stop timeout policy. It could also claim the child was
  stopped without verifying that result.
- The exception path now requests termination of its exact already-owned
  Windows Job, performs bounded root reaping, releases the root HANDLE and
  verifies Job descendant quiescence before describing the tree as stopped.
  Failure retains the cancellation/attention gate and preserves existing
  training files. Non-Windows remains unsupported and cannot claim cleanup.
- Review publication still requires `owned_tree_quiescent.is_ok()`, an
  authoritative ready-for-review worker receipt, and the existing
  cancellation-versus-completion lifecycle lock. Recovery copying is refused
  when owned child quiescence is not proven. The signed/verified checkpoint
  and automatic optimizer-resume policies were not changed.
- Added an exact-source regression contract covering the former unbounded
  wait and false-success branch. Tests were authored only; no Node, Cargo,
  Windows Job, model runtime or GPU checks were performed. An OS failure to
  terminate a genuinely unowned process remains an environment-level
  limitation, never a claimed success.

## Existing-engine inference efficiency — source-only (2026-10-15)

- **Product decision:** retain Faster Whisper/CTranslate2 ASR, MiLMMT/PyTorch
  translation, GPT-SoVITS/PyTorch TTS and the single Python worker. Research
  into alternative translation runtimes is historical only and **not an active
  candidate**; do not implement an engine swap, parallel provider, fallback
  router or model substitution for optimization.
- MiLMMT already uses persistent model reuse, BF16, deterministic generation,
  `torch.inference_mode()` and KV cache. Its `inference_ms` now includes the
  existing output-ID CPU materialization instead of stopping at potentially
  asynchronous CUDA dispatch. No extra GPU sync or speculative compiler/cache
  policy was introduced.
- Current GPT-SoVITS reuses its prompt/reference cache, including the
  TranslateIT speaker-embedding adapter. Source inspection of pinned upstream
  confirms per-`run()` `gc.collect()` and CUDA `empty_cache()` cleanup.
  Retain that lifecycle pending real multi-engine VRAM and warm-latency data;
  allocator cleanup changes are **not** proven safe from source alone.
- Optimized `voice_lab_gpt_sovits.py` PCM16 reference signal validation by
  combining silence/clipping/active-RMS/DC calculations in one traversal
  without allocating a separate active-sample list. Original quality
  thresholds and arithmetic are preserved; deterministic parity tests were
  authored but **not executed**. This affects actor/preview validation rather
  than the hot TTS acoustic generation.
- The hot-path `write_wav()` adapter no longer serializes an already
  contiguous PCM16 output with `.tobytes()`. CPython 3.12 `wave.writeframes`
  accepts a contiguous buffer directly; use `memoryview` and only make a
  contiguous copy for strided input. Existing int16/float conversion,
  mono sample format, WAV header, and empty-output rejection are unchanged.
  Source-authored byte-parity tests cover PCM16, strided and float inputs;
  these tests were not executed. Lower temporary output allocation is
  source-demonstrated, not a measured end-to-end latency improvement.
- Repeated actor-package readiness validation requires WAV duration and complete
  canonical PCM16 frames, but had called the full preview-quality analyzer
  (decode, silence, clipping, RMS and DC) each time. Both paths now share
  the original full-frame validation while duration-only reads no longer
  unpack or traverse individual samples. The same empty/truncated/noncanonical
  rejection remains. Authored negative/parity tests are not executed; this
  removes CPU work, not TTS model inference or safety validation.
- Section 5B of the **existing** target-Windows acceptance runbook now
  scopes eventual measurement to the current engines, stage latency, CUDA
  memory coexistence and selected voice output. No model/voice/GPU tests,
  cloud CI or REA runtime analysis was executed.

## Single next step

**Temporary FULL CI authorization (2026-10-18):** after each deliberate
`Local` source commit, the existing `TranslateIT Windows Local Validation`
workflow runs on `push` automatically, or can still be dispatched manually.
GitHub pins `github.sha` and cancels superseded runs so one current SHA owns
the Windows frontend, Python, Rust/Tauri source tests and aggregate proof.
This permission is temporary until the owner revokes it; automatic CI is scoped
to `Local` only, never `main`, release, schedules or PRs.

Read each exact run/job log and fix demonstrable first-owner errors without
silencing checks. A green hosted run proves source/build/test evidence only;
private models, GPU behavior, physical microphone, VB-CABLE, actual Meeting
delivery, installed app, native latency and release acceptance still require
the existing integrated `TARGET_WINDOWS` phase. The eight other quality/release
workflows remain manual-only. No cross-SHA proof composition or automatic model
inference/packaging.

**STOP boundary:** Do not start recurring cloud GPU/model jobs, change default
branch or promote app source to `main`. When the temporary CI permission ends,
restore `workflow_dispatch`-only and update this governance exception.
