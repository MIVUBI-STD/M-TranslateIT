---
id: document.knowledge.proposals.my-voice-persistent-training-architecture
class: DOCUMENT
domain: knowledge
role: GUIDE
authority: REFERENCE
lifecycle: ACTIVE
---

# My Voice V4 — Persistent Recording and Progressive Training Architecture

**Status:** PROPOSED DESIGN ONLY. This document is a reviewable proposal, not deployed behavior, product law, proof of user audio quality, or authority to alter engine weights.  
**Source baseline:** `MIVUBI-STD/M-TranslateIT` `Local` at `efc1eba28b1403212297caad89e294001fea78f0` (2026-10-13). Re-pin before any implementation.  
**User goal:** easy, efficient, entirely local voice recording and repeated improvement, with no silent overwrite/deletion of accepted recordings, existing approved actors, or model versions.  
**Non-negotiable:** retain pinned GPT-SoVITS V2ProPlus and existing Tauri/Rust authority, Svelte presentation, Python local worker, one selected Meeting voice, authorization and held-out review. No new provider/engine, hosted compute, cloud sync, auto upload, or shipped REA capability.  
**Proof ceiling:** source inspection and comparison of published open-source documentation only; no execution of REA CLI/MCP, integrated Svelte/Rust/Python, Windows device, CUDA training, actual listening quality, or backup restoration.

## 1. Evidence from current source and its limits

- `commands/voice_lab_recording/guided_lines.rs`: 128 English prompts. Existing prompt IDs and exact text are associated with `take_XXXX.wav`; mutating an old prompt in place may silently relabel a saved recording.
- `commands/voice_lab_recording/storage_transaction.rs`: accepted take replacement uses `*.wav.previous` transaction recovery; after successful replacement, old file is ordinarily deleted. This is **not** take history.
- `commands/voice_lab/storage.rs`: existing accepted takes are under `CacheData/VoiceLab/Takes`; selected/stored voice actor under `SavedProject/VoiceLab`, with transient next/previous promotion safety. The stored PersonalVoice copy is **not** an unbounded version registry.
- `commands/voice_lab_build.rs`: eligibility is 60s of accepted speech, coverage of five defined ranges, and one 3–10s reference. Build freezes a dataset, clears prior work/evaluation/candidate outputs, and after approval cleans disposable build files. Its existing actor-promotion and Meeting locking invariants must be preserved.
- `voice_lab_speaker_consistency.py`: `MAX_TAKES=128`. A prompt library of 600 does **not** mean 600 simultaneous training takes are currently supported. `evaluation.rs` also has a bounded dataset manifest (64 KiB), so limits must be redesigned and tested before increasing training volume.
- `voice_lab_gpt_sovits_build.py`: same pinned GPT-SoVITS V2ProPlus, currently SoVITS 8 epochs / GPT 15, bounded checkpoint evaluation (up to 3), held-out ASR WER, artifact flags and speaker similarity. No established safe resume-after-crash or warm-start-from-approved-actor product contract. Current candidate checkpoint cleanup must not be represented as training history.
- `docs/foundation/02-product-requirements.md`: PR-101–103 temporary build data vs approved actor storage; PR-110–119 guided exact transcripts, authorized capture, immutable baseline, human review, atomic promotion, one selected Meeting actor and no training during live Meeting. Persistent recordings and versions need an explicit product-law revision when implementation is approved.

## 2. External reference extraction, not copying whole frameworks

| Reference | Verified published pattern | Translation to a **small** local product |
| --- | --- | --- |
| GPT-SoVITS upstream, https://github.com/RVC-Boss/GPT-SoVITS | Scripted preparation, model-specific checkpoints and training | Keep pinned engine and its contracts; do not conflate inference weights with full optimizer checkpoints |
| NVIDIA NeMo Curator, https://github.com/NVIDIA-NeMo/Curator | Staged ingestion, validation, optional ASR, metrics, filtering, export manifests | Compose a bounded **advisory** quality review and deterministic dataset builder, not Ray/distributed processing |
| DVC, https://github.com/iterative/dvc | Content-derived object identity, dataset lineage and cache reuse | Immutable SHA-256 referenced audio objects + manifests, no DVC runtime dependency |
| MLflow, https://github.com/mlflow/mlflow | Independent model versions, tags, provenance and mutable deployment alias | Local `voice_version` table and single `active_voice_version_id`, no MLflow service |
| SQLite, https://www.sqlite.org/wal.html | ACID catalog transactions, WAL and recovery (with explicit durability tradeoffs) | One Rust-owned SQLite catalog on a local filesystem; no database server; WAV/model files remain external |
| ElevenLabs, https://elevenlabs.io/docs/eleven-creative/voices/voice-cloning/professional-voice-cloning | Clear record/upload/feedback/voice-use stages, user-facing audio-quality guidance | One Record / Improve / Review journey; avoid professional controls in first-time UI |
| REA, https://github.com/morluto/rea | Exact artifact evidence, bounded static/runtime observation and unknown tracking | Developer-only source-first comparison; only opt-in authorized runtime observation if source cannot answer |

No claimed vendor runtime or training result is established by this table. REA CLI/MCP was NOT EXECUTED. Do not bundle DVC, MLflow, NeMo or REA merely to imitate them.

## 3. Product invariants (acceptance blockers)

**I1 — Saved means verified.** UI says Saved only after an immutable canonical audio object, its optional native-rate partner (or explicit unavailable marker), SHA-256 verification, and committed catalog record can be reread.

**I2 — Never overwrite accepted audio.** Another recording of the same prompt creates another immutable take ID. Choosing the best take changes only a pointer; neither version deletes other audio.

**I3 — Frozen dataset for every run.** A dataset snapshot references immutable take revisions, prompt versions, exact text, content hashes and chosen reference. New captures during training do not change that run's dataset.

**I4 — Approved actors immutable.** Any new voice creates another voice version. The selected Meeting version remains unchanged until independently reviewable, human-approved candidate activation succeeds atomically with existing resource guards.

**I5 — Honest status.** Unknown filesystem/catalog/capture/model status is UNKNOWN, never silently converted to idle, success, deleted, restored, or quality-good. No invented percent-complete or absolute quality scores.

**I6 — No automatic user-data GC.** Garbage collection is permitted only for disposable *unreferenced* staging/work products after proven TTL/recovery safety. Original accepted takes, their native copies, approved versions, associated manifests and user backups are not automatically collected.

**I7 — Compatibility.** Old line IDs 1–128 and their literal source texts remain immutable; current actor format, active actor path and existing Meeting audio contracts remain valid behind a compatibility adapter. Held-out IDs 1001–1008 remain reserved and excluded from training.

**I8 — No zero-loss fiction.** Single-device failure is possible. Provide exportable and restorable verified **offline** backups, visible disk alerts, and corruption diagnostics. Never imply an unverified backup is safe.

**I9 — Privacy/consent.** Preserve current voice authorization, local-only capture and training, redacted logging and user-directed deletion/export. Saved audio lifetime and backups are clearly disclosed before collection.

## 4. Single-authority modular structure

The existing Rust `commands/voice_lab/storage.rs` remains the authoritative voice storage and promotion owner; extend it through cohesive modules rather than installing another store/controller.

| Responsibility | Owner / interface |
| --- | --- |
| Prompt catalog + stable IDs | Rust guided prompt catalog; Svelte consumes DTO, never hardcodes ranges |
| Capture/review audio | Existing Rust audio capture + `voice_lab_recording.rs` |
| Persistent audio and integrity | Rust voice storage submodule, one catalog transaction service |
| Dataset selection/freeze | Rust voice storage/build owner emits read-only dataset manifest |
| Quality analysis and training | Existing Python worker modules, output evidence only, no persistent-store writes |
| Voice version and activation | Rust voice storage/promotion, canonical selected-actor compatibility path |
| App resource arbitration | Existing ApplicationRuntime / Meeting/recording/build locks |
| Presentation | Existing MyVoice Svelte page/child components and typed Tauri bridges |
| Developer external REA | Existing optional `tools/repository_rea.py` and canonical runbook, outside app |

Do not create a second voice state manager, second poller, duplicate catalog writer, general history subsystem, or another product engine.

## 5. Local storage model: durability over novelty

**Proposed physical layout** (inside existing UserData roots, not source repository):

```text
SavedProject/VoiceLab/
  library.sqlite
  objects/audio/ab/<sha256>.wav
  voices/<voice-version-id>/{actor.json, GPT-weight, SoVITS-weight, reference.wav}
  snapshots/<dataset-id>.json
  runs/<training-run-id>/run.json
CacheData/VoiceLab/
  Draft/
  Staging/<transaction-id>/
  Working/<training-run-id>/
  Recovery/<training-run-id>/  # explicitly bounded full checkpoints if proven
```

Exact actor filenames MUST be inherited from the pinned existing actor package schema; above labels are conceptual. Use SHA-256 object identity for immutable audio and a single metadata record for source-rate/canonical pair; identical bytes can be deduplicated. Do not use symlinks as a Windows portability shortcut. Prefer one selected-actor compatibility copy unless an atomic non-copy adapter is proven safe.

**Proposed minimum tables:** `recording_take` (take_id, prompt_id, prompt_revision, text_sha256, canonical_sha256, native_sha256, metadata, accepted_at, user_status); `dataset_snapshot` (dataset_id, immutable_manifest_sha256, created_at, engine_revision, reference_take_id); `dataset_take` (dataset_id, take_id); `training_run` (run_id, dataset_id, method, config_digest, engine_digest, state, checkpoint_ref, evidence_ref); `voice_version` (version_id, actor_manifest_sha256, training_run_id, approval_id, created_at, integrity_status); `voice_selection` (single active version_id, revision); `migration_record` (legacy_path, imported_hash, transaction_id, state). Prompt catalog can remain packaged immutable data rather than being duplicated in SQL.

**File + SQLite transaction:** catalog ACID does not make filesystem WAV writes atomic. Correct save order is: write uniquely named staging bytes -> close/flush and verify canonical format+hash -> move to immutable object location without replacing another object -> commit catalog reference -> verify/reconcile on startup; incomplete stages are recoverable and never interpreted as a successful save. Select SQLite transaction/journal mode based on Windows durability test; WAL with `synchronous=FULL` is a candidate but does not itself eliminate all disk/hardware failure modes. Backups use SQLite's online backup API, plus referenced audio/model objects and a signed/hashed manifest, then restore into a separate staging library and verify dependencies before cutover.

**Space optimization:** reference immutable audio by hash instead of copying all takes into multiple permanent datasets. Copy into disposable GPT-SoVITS staging only where legacy worker requires a flat WAV directory. Versioned actor package may need a single active compatibility copy; monitor size. Cache preprocessing keyed by content + engine revision + parameters may be reused only after proof of identical semantics. Limit retained *recovery checkpoints* separately from permanent actor versions.

## 6. Backwards-compatible migration

M0: detect existing `CacheData/VoiceLab/Takes`, `NativeRate`, `SavedProject/VoiceLab/MyVoice` and `PersonalVoice` without modification. Resolve legacy line-id -> exact historical text from pinned catalog v1.

M1: copy and hash each valid accepted legacy take to immutable audio store, including native sidecar when valid; create deterministic import transaction identities and explicit missing-sidecar status. NEVER rename/delete original at this stage.

M2: register existing approved and separately saved personal actor as immutable versions after actor package validation; retain currently active voice pointer and selected legacy runtime until adapter proves equivalent.

M3: verify count, exact text binding, hashes, origin paths, duplicate handling, selected actor, restart recovery and power-failure simulations. A failed or partially imported file stays visible as `migration_attention` and is not silently skipped.

M4: enable new read path only after a complete verified migration and explicit activation contract. Keep legacy files as read-only recovery evidence until user authorizes a later cleanup; a copy is not backup if it lives on the same failing disk.

Do not run dual writers; one canonical Rust store owner. If migration fails, preserve old source, old active voice, and old app behavior.

## 7. Prompt library and coverage

Target an extensible reviewed library of ~600 English prompts, with user-facing **Short Conversations / Natural Conversations / Detailed Explanations** (or equivalent concise Indonesian localization), not `easy=good / hard=better`. Keep hidden orthogonal tags: phoneme/syllable-pattern coverage; connected speech; stress and intonation; question/contrast; numbers, names, acronyms; meeting vocabulary; natural short/medium/long speech. No requirement to record 600.

Existing `line_id` 1–128 remains bound to immutable v1 text. New entries receive stable `prompt_id` with explicit `training` or `evaluation` namespace; do not reuse 1001–1008 held-out numeric IDs. Adapter may expose legacy numeric IDs during migration but new storage keys are namespace-aware (`training:00129`, etc.). A revised spelling/text gets a NEW prompt revision while old recordings retain their original transcript identity.

Generate final prompt set through corpus review and coverage diagnostics, not arbitrary count. Remove near-duplicate prompts; constrain readability, length, language, and naturalness. User-facing category is independent from evidence tags. Existing known guided text is authoritative; optional ASR may only advise replay, never overwrite transcription automatically.

**Selection:** user can choose any prompt; default suggests one that adds unrecorded categories or underrepresented patterns. Use deterministic, explanation-friendly ranking without a fabricated `voice quality 92%`. One best accepted take per prompt in a run by default. A 600-item *catalog* does not authorize 600-item *train*. Keep current 128-take limit until performance and bounds are engineered; make batch selection deliberate, preserve all takes outside the run, and test larger limits + manifest sizes before promotion. Avoid quadratic pairwise speaker comparisons over full user archive.

## 8. Run lifecycle and three kinds of continued work

User-facing term: **Improve My Voice**. For the first safe implementation it means `REBUILD_WITH_MORE_DATA`, using a new frozen dataset that reuses selected old+new takes and the same pinned pretrained engine.

Internally separate:
- `REBUILD_WITH_MORE_DATA`: fresh fine-tune from unchanged pinned pretrained weights; old voice immutable; full or curated composite dataset; new candidate and version.
- `WARM_START_FROM_VOICE`: future opt-in experiment initializing compatible training from prior actor weights; compare against rebuild on identical dataset; guard forgetting/overfit; do not assume inference weights contain optimizer state.
- `RESUME_INTERRUPTED_RUN`: continue exact same run only with full optimizer/scheduler/step/checkpoint state and matching hashes of dataset/config/engine. Changed dataset => new run, not resume.

States: `draft → prepared → queued → training → evaluating → review_ready → approved_version → selected`; failures `failed / interrupted / cancelling / needs_recovery` preserve resources and never clear old active. A run ID owns its staging and cancellation; its immutable manifest cannot be edited during execution. Restart reconciliation is performed by Rust, not Svelte guesses.

GPT-SoVITS engine revision, epochs and candidate ranking stay fixed in first release, so any observed quality changes are attributable to data/flow rather than mixed training changes. Preserve eight held-out evaluations and exact review-ID/WAV hash checks. Human may reject new candidate without damaging existing actor. Later A/B blind comparisons can add naturalness/identity checks but no arbitrary absolute quality grade.

## 9. First-time UI/UX contract

**Main My Voice page:** selected Meeting voice/version + one dominant contextual action (`Record more`, `Create My Voice`, `Improve My Voice` or `Review new version`) + small progress summary. Advanced links `Saved recordings`, `Voice versions`, `Storage & backup` are discoverable but do not compete.

**Record view:** one prompt and a clear Record/Stop/Replay/Save this recording control; group filters for Short/Natural/Detailed, optional category browsing, recommended next line with reason, skip without penalty; previously saved takes indicated without suggesting mandatory completion. After save, show **saved on this device** only on verified receipt.

**Improve view:** explain new recordings available, what dataset version will be used, that current voice is protected, and truthful phase labels. No fake percent/ETA, no hidden active Stop/Retry, no need to understand epochs or checkpoint file names.

**Review view:** the same eight held-out texts, play-to-completion gating and consent. Optional listen to current vs candidate using identical text and loudness handling validated for fairness. `Use new version` is explicit; `Keep current voice` is always safe and accessible.

**Versions view:** Version 1/2/etc., created date, source dataset, approved/selected/candidate, last verification, Restore/Select when safe; never allow delete of active/referenced last recovery version without explicit informed action. No invisible `best version` automatic switch.

**Storage view:** local location, estimate of recordings/models/cache, `Back up My Voice`, `Verify backup`, `Restore from backup`, `Clear temporary training files`. Raw data deletion separated from cache clearing. User-readable failures state which items remain safe.

## 10. REA investigation matrix and evidence ceiling

Use canonical `docs/knowledge/operations/rea-investigation.md` and direct `morluto/rea` contracts. Source-visible open projects: read pinned GitHub source first and link specific implementation functions; do **not** execute REA ceremonially. Runtime/packaged competitor: requires owned or expressly authorized artifact/profile and a capable local host; begin passive target-scoped capture, not downloading or decompiling copyrighted binaries without authority.

| Hypothesis | Evidence to request when target authorized | Acceptable transfer |
| --- | --- | --- |
| Competitor recording state survives page/app restart | Bounded current-origin accessibility DOM + permitted change-window capture | State transition and user recovery copy, not hidden backend assumption |
| Competing voice versions are selectable/rollback safe | Published SDK contract or authorized UI observation; failure/crash simulation only on own app | User behavior invariant |
| Browser auto-save provides real durable acknowledgment | Client-visible contract and explicit test on own app | Never infer durability from a toast |
| Packaged app exposes incremental training controls | Published docs/source; selected runtime observation only if needed | Separate resume / warm start / rebuild terms |

Record artifact identity, scope, source path or Evidence ID, confidence and UNKNOWNs. **THIS DESIGN:** public source/document inspection only; actual REA CLI/MCP, controlled browser observations, binaries and model tests **NOT EXECUTED**.

## 11. Implementation partition and falsifiable acceptance

**A — Product/storage authority:** amend PR-101–103/110–119 for opt-in durable recordings, retention/export and versioning; one storage owner; define immutable formats and risk policy. Source contract acceptance: legacy files never touched on speculative initialization.

**B — Persistent recording storage + migration:** schema, object store, save transaction, idempotent recovery and read-only migration. Source tests: duplicate take ID, same prompt retakes, disk full, missing sidecar, power interruption at every transaction boundary, checksum mismatch, rollback without losing old take, app restart.

**C — Corpus/coverage/dataset snapshot:** stable prompt revision/metadata, ~600 reviewed prompts, advisory coverage selection, deterministic frozen run manifests, 128-take ceiling explicitly handled rather than secretly lifted. Source tests: 1–128 old text immutable, 1001–1008 held out, no duplicate IDs/text, same manifest => same digest, late captures excluded.

**D — Voice registry and activation:** immutable version packages, verified selected voice alias, atomic swap using existing Meeting guard and compatibility path, A/B playback. Source tests: failed promotion retains old voice, candidate rejection retains old version, repeated fast switch and active Meeting rejection.

**E — Improvement and recovery:** rebuild from old+new frozen data; later full checkpoint resume and experimental warm start isolated by method identifiers. Test exact dataset/engine hash mismatch, interrupt/restart/cancel, retention bounds and optimizer-state identity.

**F — Integrated native acceptance (deferred at user's request):** exact-SHA Svelte/Node/Cargo/worker source+compile checks, clean Windows install/update and migration, physical mic, RTX-class fine-tuning/VRAM, 8 sample listening, concurrent resource guards, export/import from another path/drive, corruption/disk-full/power loss simulations and Meeting output. Until performed, say `SOURCE_IMPLEMENTED / RUNTIME_NOT_EXECUTED`, not production-ready or audio-quality proven.

**Acceptance conditions**: 1) every newly accepted take is independently restorable and no prior accepted file is overwritten; 2) all versioned actor and dataset references remain integrity-checkable across restart; 3) user can reject or interrupt an improvement without changing active Meeting voice; 4) users can record and improve without understanding training jargon; 5) off-device backup is exportable and restore-tested before any claim of disaster recovery.

## 12. Architecture decisions still needing evidence

- Choose SQLite Rust dependency carefully; existing `Cargo.toml` does not already include `rusqlite`, so dependency/lock/release impact and exact ownership must be evaluated before adoption. Do not silently add a library just because it is familiar.
- Whether originals live by default in `SavedProject` or require a one-time privacy/storage opt-in; preserve existing TEMPORARY law until formally revised.
- Recovery-checkpoint max bytes and count, disk reserve thresholds and user controls need target-Windows size/throughput measurements.
- Final prompt count and training take cap follow evidence, not `600 means train everything`.
- Inference-package cache duplication and update/migration backup format must be measured and crash-tested on NTFS.
- No user-entered data, voice audio, full logs or proprietary third-party captures belong in Git history.

**Next bounded decision:** approve a single Rust-owned durable recording and actor-version contract, plus migration behavior and consent/retention defaults. Then integrate one coherent source implementation in existing owners. Maintain Meeting V2 as a separate active work track; this proposed document is not a competing active continuation ledger.


## Source implementation addition — immutable snapshots and model version foundation

A subsequent source candidate adds Rust-owned modules under \`commands/voice_lab/storage/\`:
\`immutable_store.rs\`, \`dataset_history.rs\`, and \`actor_history.rs\`.
The frozen dataset still lives in \`CacheData/VoiceLab/Build/Dataset\` for
the pinned worker, but its immutable manifest and referenced audio are kept in
\`SavedProject/VoiceLab/DatasetSnapshots\` and \`Recordings\`. A new
\`snapshot.ref\` pointer in the disposable dataset directory links the two.

Approved actor promotion and personal voice replacement first preserve immutable
whole-package versions under \`SavedProject/VoiceLab/VoiceVersions\`. When a
dataset reference exists, a separate content-addressed \`TrainingLineage\`
record binds the reviewed actor version to the dataset snapshot. Models with
unknown legacy provenance remain explicitly unlinked. Partial, corrupt or
unverified version archives block destructive replacement.

These are **storage foundation source candidates**, not a complete training
resume feature, current-voice restore UI, verified backup to another disk,
or measured acoustic improvement. Rust/Windows, GPU, audio and real recovery
execution remain **NOT EXECUTED** for this source revision.
