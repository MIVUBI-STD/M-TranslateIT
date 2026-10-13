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


## Source implementation continuation — Voice Version Browser and guarded selection

The existing immutable actor package registry now exposes a bounded
read-only, metadata-only \`VoiceVersions\` page. Listing never claims full
weight validation. Selection reuses \`voice_selection_guard\`, Meeting/recording/
training/Quick Preview checks, and Tauri ApplicationRuntime events. Explicit
confirmation is required. The Rust storage owner verifies complete GPT,
SoVITS, reference and manifest hashes, retains the currently selected personal
actor as an immutable version, copies to guarded staging and uses the existing
selected-voice atomic swap owner. Pending/ambiguous previous transactions block
further replacement; unknown frontend results never become success.

The Svelte \`VoiceVersionBrowser\` is a secondary disclosure and has no
independent selected-voice state owner or polling loop. Its list labels
"verification required" and does not invent dates, training scores, or
version chronology based on content hashes. The saved PersonalVoice copy is
reconciled after selection and any failure is disclosed as an attention state.
This is not warm-start training, checkpoint resume, model-quality acceptance,
or externally backed disaster recovery.

**Proof limit:** this work is only REMOTE_GITHUB source-reviewed until an
exact-SHA Windows Cargo/Svelte/Node and native version-switch regression run.


## Source implementation extension — Improve My Voice archived rebuild

A new explicit user-authorized \`start_product_voice_improvement\` command
reuses the canonical Rust-owned \`start_voice_lab_build_selected\` and existing
Python GPT-SoVITS V2ProPlus training worker. The default \`Create My Voice\`
path remains unchanged. The saved-recordings disclosure now allows choosing
exactly one archived take per guided sentence and creating a new candidate.

The archive selection requires verified source bytes, exact immutable guided
text, at least 60 seconds of accepted speech, five existing coverage groups,
a 3–10 second reference candidate, and at most 128 selected takes per run.
Every selection is ordered by line ID, copied into the existing flat WAV
worker dataset, and bound to its own permanent content-addressed snapshot.

Held-out evaluation for this explicitly archive-backed run verifies the exact
frozen WAV and saved archive bindings, **not the current mutable active-take
paths**. This allows accepting more new recordings without invalidating an
already completed archived training review. Legacy non-archive review retains
its original current-take byte comparison. Failure is fail-closed; there is no
silently inferred dataset source.

The user-facing action is \`Create improved version\`, but outcome quality is
still unknown until a new candidate completes all existing held-out checks
and human approval. This is \`REBUILD_WITH_MORE_DATA\`, **not** true optimizer
resume, checkpoint warm start, or a claim that more audio always improves
speaker identity. Actual Windows training, integrated frontend, GPU resource
use, acoustics and source tests remain **NOT EXECUTED**.


## Source implementation continuation — Corpus V3 (600 English prompts)

The corpus expansion preserves all 128 original ID/text pairs unchanged and
extends them with 472 independently authored, non-duplicated prompts:

| User-facing level | IDs | Existing | Added | Total |
| --- | --- | ---: | ---: | ---: |
| Short Conversations | 1–24 and 129–284 | 24 | 156 | 180 |
| Natural Conversations | 25–96 and 285–482 | 72 | 198 | 270 |
| Detailed Explanations | 97–128 and 483–600 | 32 | 118 | 150 |

New prompts have orthogonal **level**, **focus** (32 distinct newly authored
tags), and **coverage** (conversation, intonation, natural speech, details,
explanations) metadata in the Rust source. The legacy lines receive derived
metadata without changing source transcript characters. Held-out evaluation
IDs 1001–1008 are never training prompts. The guidance algorithm now checks
semantic coverage tags instead of hardcoded legacy ID intervals.

The UI uses those backend-provided metadata properties, filters by reading
level and focus, supports text/ID search, and renders 35 visible rows per page.
It also offers a missing-category suggestion, without presenting the
number of completed prompts as a speaker-quality percentage. Users may select
any comfortable sentence, in any order, and leave the session with saved
recordings.

Source-only corpus checks at authoring found 600 sequential IDs,
**zero exact duplicate texts**, 32 new focus tags, 1,847 distinct lexical
word tokens and an average of approximately 12.2 written words per prompt.
These are lexical/count sanity checks, **not** measured TTS acoustic,
perceptual or ARPABET/diphone coverage. There is no claim that 600 prompts
are required or guarantee higher voice quality. All prompts are new authored
sentences or retained original TranslateIT texts; no third-party dataset
passages are imported. Published Common Voice and L2-ARCTIC guidance informed
the natural-speech, flexible-accent and varied-phonetic design principles;
their licensed corpora and voice files are not bundled.

The independent code-level worker limit remains 128 accepted takes per
training run. If the user records more than that, ordinary Create refuses an
oversized run with guidance to select up to 128 from Saved Recordings. The
existing Improve My Voice saved-take selection can use any prompt in this
600-item library while preserving the exact frozen transcript. This prevents
silent truncation, a hidden random training subset, and unsupported
speaker-consistency workloads.

**Proof:** REMOTE_GITHUB source inspection and authored regression tests.
Actual Rust/Svelte/Node build, Windows audio runtime, acoustic phoneme
distribution, listening similarity and training performance remain
**NOT EXECUTED**. Reference context:
https://commonvoice.mozilla.org/en/guidelines and
https://psi.engr.tamu.edu/l2-arctic-corpus/.


## Source implementation continuation — optional phonetic-aware guidance

The Rust My Voice guided catalog has an additional static text-derived
pronunciation inventory. A developer-only standard-library Python script
\`tools/generate_my_voice_phonetics.py\` takes an explicitly supplied,
offline CMUdict dictionary file and regenerates the 600-entry inventory
without installing a package or accessing the network. The pinned upstream
CMUdict Git blob is
\`2c0411740cce3e2026a80b90b650d5f6a7258164\`. Source:
https://github.com/cmusphinx/cmudict.
CMUdict is Copyright (C) 1993–2015 Carnegie Mellon University; attribution
and redistribution terms: https://github.com/cmusphinx/cmudict/blob/master/LICENSE.
No dictionary text is copied into TranslateIT's shipped product; generated
per-line bitsets hold an FNV64 text fingerprint, a 39-symbol unstressed
ARPABET presence mask, 64 selected within-word symbol-pair bits, and the
known-word fraction. The GitHub-reviewed generation pass recognized ~99.81%
of tokens among the 600 English sentences with that US-English dictionary.
This is NOT measured pronunciation accuracy, accent conformity, intelligibility
or quality of any generated voice. Homographs, speaker accents, rhythm and
expressive intonation cannot be inferred reliably from these bitsets.

The low-cost deterministic Rust recommender computes masks over already
accepted line IDs, ranks unused guided prompts by new text-estimated sound
patterns, and can prioritize a missing legacy-compatible speech category.
It intentionally never modifies take status, training datasets, minimum
requirements, voice versions or Meeting voice selection. Results are capped
at three suggestions; the Svelte UI presents the prompt and reason in the
existing recording progress area without a separate runtime worker or
additional status-polling owner. Previously accepted recordings remain
immutable. A fingerprint mismatch disables phonetic suggestions rather than
silently attaching the wrong annotation to edited prompt text.

Upstream GPT-SoVITS English G2P/CMUdict references:
https://huggingface.co/spaces/lj1995/GPT-SoVITS-ProPlus/blob/main/text/english.py .
No engine, model weights, inference behavior or training epochs are changed.
Actual English phone coverage is not fully measured: only the 39
unstressed dictionary phoneme symbols and a fixed subset of 64 within-word
transitions were tracked, not all diphones, contextual prosody or a speaker's
real acoustic production. Full human review remains a separate acceptance
gate. The previous source-only proof limit still applies: Cargo/Svelte/Node,
Windows mic, training quality and native crash recovery NOT EXECUTED.


## Source implementation continuation — portable verified My Voice backup

The Rust-owned \`voice_lab/storage/backup.rs\` adds a bounded, explicit
folder-format backup, containing only immutable:
- \`Recordings\`
- \`DatasetSnapshots\`
- \`TrainingLineage\`
- \`VoiceVersions\`

Before exporting, the current and separately saved personal actors are
preserved as immutable package versions. Recognized legacy cached WAVs and
\`*.previous\` recovery takes are protected through the existing copy-only
migration. A previous incomplete voice swap stops export instead of silently
guessing which package to keep.

The user selects an **existing absolute folder**. TranslateIT creates a unique
\`TranslateIT-MyVoice-<timestamp>\` child directory via verified staging, with
\`data/\` and \`my-voice-backup.json\`. The manifest names only whitelisted,
portable archive paths and includes bounded file sizes and SHA-256 hashes.
An exact backup can be verified without restoring; verification checks file
hashes and references between recordings, datasets, lineage, and model files.
The format is not a signature or authenticity guarantee.

Restore is **additive-only**: it never changes the currently selected Meeting
voice, never overwrites existing saved archives, and never alters active cache
takes or audio devices. Existing matching files are reused, conflicting files
block the import in preflight, and newly imported immutable files are written
through per-file staging with verification. Interrupted valid staging is
recoverable; corrupt partial staging is reported rather than deleted. The
user may then explicitly choose an imported personal voice using the existing
Safe Restore mechanism and human approval.

The secondary \`Storage & backup\` UI provides Create, Verify, and Import.
Without an installed native file-dialog dependency, this iteration accepts
user-pasted absolute folder paths from File Explorer; it does not silently
write to or upload an unknown provider and warns users to store a backup on
another drive. This is a bounded UX compromise rather than a permanent
folder-picker design.

**Source-only proof:** the GitHub edit includes Windows-scoped native fixture
tests and Node source-contract tests, but they have NOT EXECUTED. The
packaged Windows filesystem, actual external drive/NTFS behavior, multi-file
crash recovery and copied model usability still require integrated target
acceptance. Do not infer production-ready disaster recovery from a committed
source module or self-consistent manifest.


## Source implementation continuation — optimizer checkpoint preservation before true resume

**Pinned upstream audit:** GPT-SoVITS commit
\`d523079fc05d9a8028d6085bffe4a2757c32abb6\`.
Source:
https://github.com/RVC-Boss/GPT-SoVITS/blob/d523079fc05d9a8028d6085bffe4a2757c32abb6/GPT_SoVITS/s1_train.py
and
https://github.com/RVC-Boss/GPT-SoVITS/blob/d523079fc05d9a8028d6085bffe4a2757c32abb6/GPT_SoVITS/s2_train.py .
GPT uses PyTorch Lightning \`ModelCheckpoint\` under
\`experiment/logs_s1_v2ProPlus/ckpt/\` and \`trainer.fit(...ckpt_path=...)\`;
these are different from lightweight candidate inference \`*-eN.ckpt\`.
SoVITS \`utils.save_checkpoint\` saves \`model\`, \`optimizer\`,
\`learning_rate\`, \`iteration\` for G and D under
\`experiment/logs_s2_v2ProPlus/\`; it uses fixed latest-save filenames
\`G_233333333333.pth\` and \`D_233333333333.pth\` when
\`if_save_latest=True\`. Critically, SoVITS uses a broad \`except\` to fall
back to pretrained G/D when loading fails, reconstructs scheduler by stepping
from loaded epoch, and does not save its AMP \`GradScaler\` in the full
checkpoint. Therefore checkpoint file presence does NOT prove exact
optimizer/scheduler/precision resume.

The new Rust-owned
\`commands/voice_lab/storage/training_recovery.rs\` stores immutable
\`SavedProject/VoiceLab/TrainingRecovery/<digest>/\`
content-addressed archives, including available pinned GPT full checkpoint(s),
SoVITS G/D full checkpoint(s), frozen dataset reference, and exact training
config files. It streams file copies and hashes, never deletes cache/source
state, verifies saved copies and refuses to pair old checkpoints with a new
dataset snapshot. Capture is invoked after failed/cancelled worker exit and
BEFORE new dataset preparation/old Work cleanup. No checkpoint found means
no completed epoch state to preserve; partial/uncheckpointed computation cannot
be reconstructed magically. Failed capture blocks cleanup and remains an
explicit attention issue rather than silent progress loss.

The backup serializer now includes TrainingRecovery objects, with strict
relative path whitelisting and reference integrity checks tying recovery
receipts to their immutable dataset snapshots. A lightweight read-only
recovery count is displayed under Storage & Backup, explicitly saying
**automatic Resume is not yet available**. Full weight verification belongs
to backup/import and future resume preflight, not every UI status read.
Neither GPT-SoVITS engine weights, pinned upstream training logic, inference,
Meeting voice nor the original user recording texts were modified.

**Evidence ceiling:** source-authored and statically inspected only in
REMOTE_GITHUB. Exact Windows Rust/Node/Svelte builds, cancellation recovery,
disk-full tests, actual G/D/GPT checkpoint deserialization, optimizer-state
continuation, scheduler/scaler continuity and real speech quality are NOT
EXECUTED. Next decision is whether a bounded fail-closed training-runner
adapter can prove strict SoVITS loading without modifying the pinned model
engine. Until then, do NOT activate a user-facing Resume.


## Source implementation continuation — strict read-only resume validation

A bounded developer-only module \`EngineData/Backend/LocalWorker/WorkerRuntime/voice_lab_resume_validation.py\`
performs an explicit **read-only** admission audit of one
\`SavedProject/VoiceLab/TrainingRecovery/<SHA256>\` object. It validates:
- Full recovery receipt hash, strict version and source dataset ID.
- Frozen dataset manifest and exact archived take/transcript references.
- Bounded SHA-256 streaming of every checkpoint/config file and canonical WAV.
- Pinned upstream GPT-SoVITS source marker plus exact Git blob identities
  (GPT \`s1_train.py\`, SoVITS \`s2_train.py\` and \`utils.py\`).
- Presence and filename pairing of GPT full checkpoints, SoVITS Generator and
  Discriminator states and their exact saved training configurations.
- Optionally, with \`--inspect-tensors\` on **trusted local files only**, safe
  PyTorch \`weights_only=True\` inspection of GPT epoch, global step,
  optimizer/scheduler and SoVITS G/D model/optimizer/iteration fields, without
  a fallback to \`weights_only=False\` if loading fails.

Example *local developer command*, NOT executed through GitHub:
\`\`\`sh
python EngineData/Backend/LocalWorker/WorkerRuntime/voice_lab_resume_validation.py \
  --saved-root "<user-data>/SavedProject/VoiceLab" \
  --recovery-id "<64-character checkpoint archive ID>" \
  --source-root "<installed-pinned GPT-SoVITS source>"
\`\`\`
The optional \`--inspect-tensors\` switch is reserved for already-owned,
trusted local checkpoints and can require CPU memory proportional to checkpoint
size; the default check is stdlib-only and streaming. No CPU/GPU inference
or training is launched by this inspector, and no recovery data is modified.

Its result intentionally always reports \`resume_available=false\` because
static optimizer structural inspection is not proof that the *trainer actually
loaded* the optimizer or continued from the prior epoch. The pinned upstream
SoVITS loader's broad \`except\` can silently start from pretrained weights,
and its full save path lacks AMP \`GradScaler\` persistence and reconstructs
scheduler state. A future strictly acknowledged runner must fail on fallback,
check the actual resumed epoch/optimizer state, preserve dataset/config/model
pins and prove the behavior on target Windows before enabling an end-user
Resume action. Separate rebuild-from-saved-recordings is already available;
it is not advertised as optimizer resume.

**Verification level:** source-reviewed module and independently authored
unit fixtures exist on GitHub, but no Python/Cargo/Svelte execution, upstream
checkpoint deserialization, native Windows model training or acoustic
evaluation was performed during this source-only implementation turn.
