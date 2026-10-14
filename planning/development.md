# Active development — My Voice source stabilization (2026-10-14)

**State:** SOURCE_CANDIDATE. My Voice source hardening is the current user-selected priority. No source-only change may be reported as compiled, Windows-tested, or audio-quality accepted.

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
- Developer-only GPT/SoVITS checkpoint probes remain isolated. User Resume remains **DISABLED** because SoVITS scaler/scheduler continuity, cross-worker provenance and actual Windows GPU restoration are not established. Avoid expanding this experiment while the primary flow lacks native acceptance.
- `docs/knowledge/current-validation.md` owns evidence interpretation. No exact-`Local`-HEAD Svelte/Node/Cargo/Python build/test, real personal-voice training, Windows backup recovery, perceptual voice comparison or Meeting receiver proof is available for this source batch.

## Verification boundary

This phase is `REMOTE_GITHUB` only: review exact source and write bounded regression tests; do not dispatch hosted CI, claim test execution, request incremental PC testing or touch `main`. Once the integrated source candidate is coherent, the existing `docs/knowledge/operations/target-windows-performance.md` will govern a single later local build/package/Windows acceptance phase.

## Single next step

**Audit accumulated My Voice source changes against existing Svelte, Rust and Python contracts, correct the first verifiable mismatch, and prepare one integrated exact-SHA local acceptance run.** No hosted CI, incremental user-PC testing, extra voice engines or checkpoint-Resume activation.

## Deferred prior work

Meeting V2 Phase 0 remains a separate unvalidated source candidate. Its continuation and evidence limits are preserved in prior `Local` commits and `docs/knowledge/operations/target-windows-performance.md`; do not treat older Meeting proof as acceptance of My Voice or silently resume Meeting work during this My Voice pass.
