# Active development — My Voice source stabilization (2026-10-14)

**State:** SOURCE_CANDIDATE. My Voice source hardening is the current user-selected priority. No source-only change may be reported as compiled, Windows-tested, or audio-quality accepted.

**Authority:** `MIVUBI-STD/M-TranslateIT`, branch `Local`. Pin the actual `Local` HEAD for each read/write and all future executable acceptance. The 2026-10-12 partial developer checks on `230ca94faae178659a7738fa82399485c267c991` do not certify newer My Voice revisions.

## Current scope and source truth

- Preserve the current Tauri/Svelte + Rust storage owner + one Python worker, GPT-SoVITS V2ProPlus revision, 600 guided English prompts and the **128-take maximum per build**. The larger prompt catalog is not a promise to train 600 takes at once.
- Keep ordinary recording, Quick Preview, frozen-dataset Create, eight held-out review samples, explicit approval/Meeting selection, saved-version Improve (rebuild), and immutable backup as the first-release target.
- Preserve previous saved recordings and selected Meeting voice; unknown storage/worker state fails closed. Backup verification requires receipt-to-payload **and payload-to-receipt** linkage and content-addressed receipt identity; unreferenced archive bytes are not restorable objects.
- Ambiguous Start/Stop/Retry/Accept recording outcomes keep recording coordination locked until a fresh Rust status read. Accepted-take consumers refresh after uncertain Accept without replaying its write.
- Developer-only GPT/SoVITS checkpoint probes remain isolated. User Resume remains **DISABLED** because SoVITS scaler/scheduler continuity, cross-worker provenance and actual Windows GPU restoration are not established. Avoid expanding this experiment while the primary flow lacks native acceptance.
- `docs/knowledge/current-validation.md` owns evidence interpretation. No exact-`Local`-HEAD Svelte/Node/Cargo/Python build/test, real personal-voice training, Windows backup recovery, perceptual voice comparison or Meeting receiver proof is available for this source batch.

## Verification boundary

This phase is `REMOTE_GITHUB` only: review exact source and write bounded regression tests; do not dispatch hosted CI, claim test execution, request incremental PC testing or touch `main`. Once the integrated source candidate is coherent, the existing `docs/knowledge/operations/target-windows-performance.md` will govern a single later local build/package/Windows acceptance phase.

## Single next step

**Audit Create → Review → Approve → Meeting transitions on exact current `Local` source; repair the first source-provable build/approval/selection failure with focused regression coverage.** Preserve existing listening and consent gates; avoid UI redesign, new engines, and Resume expansion.

## Deferred prior work

Meeting V2 Phase 0 remains a separate unvalidated source candidate. Its continuation and evidence limits are preserved in prior `Local` commits and `docs/knowledge/operations/target-windows-performance.md`; do not treat older Meeting proof as acceptance of My Voice or silently resume Meeting work during this My Voice pass.
