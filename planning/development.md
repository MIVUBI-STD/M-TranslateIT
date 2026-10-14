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
- Source contract audit repaired two stale Quick Preview assertions: the displayed prompt total follows the 600-line catalog dynamically while the build cap stays at 128; held-out `Listened` proof follows the current audio instance/generation/review ID rather than the previous callback shape. The assertions were inspected, not executed.
- Exact selected-version identity uses an **explicit, read-only** Rust verifier: it streams SHA-256 over the actual selected actor package and compares its content-addressed receipt digest to the requested version ID. No heavyweight hash enters frequent status polling. Ambiguous restores stay locked when identity is **unknown**, but an explicitly verified mismatch (`false`) releases the lock without claiming the requested version was restored. This distinguishes match/mismatch/unavailable and avoids permanent lockout when the earlier Meeting voice is intact. Native execution remains pending.
- The explicit exact-version hash command now runs on Tauri's blocking worker instead of the UI thread, and rechecks Meeting/recording/build/Quick Preview ownership after acquiring the existing voice-selection guard. No heavy hashing is added to status polling. Node regression coverage is authored but unexecuted.
- Backup integrity now also binds the original immutable `DatasetSnapshots/<sha>.json` and `TrainingLineage/<sha>.json` path IDs to the SHA-256 of their actual content. This closes a gap where the backup's own index could be updated to match forged archive metadata, while the immutable history store would later reject the imported object. Added negative Rust fixtures; execution remains untested.
- Backup semantic validation now checks each frozen DatasetSnapshot's original typed source manifest (authorization, take IDs/names and held-out separation), its audio-link cardinality, and each link's exact archive receipt line/text binding. TrainingLineage metadata must use the supported schema and engine revision before its references can pass. These source-only regressions are authored, not executed; the backup import still never activates a Meeting actor.
- VoiceVersions backup verification now additionally checks the immutable actor receipt schema/revision, recorded payload sizes, and the existing Rust actor package validator before declaring a version restorable. This avoids accepting self-consistent backup hashes that could not be activated as MyVoice. Regression fixtures for unsupported version receipts are authored; native verification is unexecuted.
- Developer-only GPT/SoVITS checkpoint probes remain isolated. User Resume remains **DISABLED** because SoVITS scaler/scheduler continuity, cross-worker provenance and actual Windows GPU restoration are not established. Avoid expanding this experiment while the primary flow lacks native acceptance.
- `docs/knowledge/current-validation.md` owns evidence interpretation. No exact-`Local`-HEAD Svelte/Node/Cargo/Python build/test, real personal-voice training, Windows backup recovery, perceptual voice comparison or Meeting receiver proof is available for this source batch.

## Verification boundary

This phase is `REMOTE_GITHUB` only: review exact source and write bounded regression tests; do not dispatch hosted CI, claim test execution, request incremental PC testing or touch `main`. Once the integrated source candidate is coherent, the existing `docs/knowledge/operations/target-windows-performance.md` will govern a single later local build/package/Windows acceptance phase.

## Single next step

**Audit remaining Create → Review → Approve ownership and exact saved-actor preservation, then perform final static regression-contract admission for the pinned Local source. Do not expand product scope. If coherent, stop source mutation and prepare the single integrated Windows acceptance run using the existing runbook.** No test PASS without execution, hosted CI, alternate voice engines or optimizer Resume activation.

## Deferred prior work

Meeting V2 Phase 0 remains a separate unvalidated source candidate. Its continuation and evidence limits are preserved in prior `Local` commits and `docs/knowledge/operations/target-windows-performance.md`; do not treat older Meeting proof as acceptance of My Voice or silently resume Meeting work during this My Voice pass.
