---
id: document.foundation.product-requirements
class: DOCUMENT
domain: foundation
role: CONTRACT
authority: CANONICAL
lifecycle: ACTIVE
---

# TranslateIT — Product Requirements

**Status:** Active Policy  
**Updated:** 2026-09-07  
**Scope:** Windows local-first Indonesian ↔ English translator; built-in Meeting voices + optional My Voice

This file contains **current law only**. Superseded choices belong in `docs/knowledge/decisions/history-legacy.md`, not as competing active requirements here.

## 1. Product priority

### PR-001 — Primary use case
**MUST:** TranslateIT primarily support simple Indonesian ↔ English translation for online meetings.

### PR-002 — Text
**MUST:** Text remain independently usable without Meeting audio or My Voice.

### PR-003 — Reliability before breadth
**MUST:** Translation completeness, meaning, safe delivery and truthful readiness outrank extra features/modes.

### PR-004 — Product boundary
**MUST:** Normal top-level product remain `Meeting | Text | My Voice | Settings`.

**NOT CURRENT CORE:** Document Translation, general History/Saved UI, Audio Studio, Push to Talk, Pause/Resume, Realtime/Quality user modes, Auto/Casual tone styles, additional languages, partial translated subtitles, incoming Indonesian TTS, imported-audio/quick-clone My Voice, multiple normal voice engines/providers. Style selectors, Spoken Terms, Preferred Words, Meeting Presets and Text floating captions are retired from active product behavior.

## 2. Platform and runtime

### PR-010 — Platform
**MUST:** Windows is the current supported/validated platform.

### PR-011 — Local-first
**MUST:** After required assets are installed, ASR, translation, Meeting voice synthesis and Text operate without a required cloud speech/translation/voice API.

### PR-012 — No silent cloud fallback
**MUST NOT:** Local failure silently route speech/text/voice data to cloud services.

### PR-013 — One runtime architecture
**MUST:** One Tauri/Rust desktop application and one canonical Python worker own normal inference. GPT-SoVITS V2ProPlus remains the approved voice engine family for built-in/My Voice behavior until explicitly replaced.

**MUST NOT:** Add a second normal translator/TTS engine, provider router, shell or compatibility runtime merely to avoid solving an integration defect.

## 3. Languages and directions

### PR-020 — Languages
**MUST:** Support Indonesian ↔ English only.

### PR-021 — Text directions
**MUST:** Text support ID→EN and EN→ID.

### PR-022 — Required outbound Meeting flow
**MUST:**

```text
Indonesian speech
→ final Indonesian transcript
→ English translation
→ selected Meeting voice (Built-in or approved My Voice)
→ TranslateIT Meeting Microphone
```

### PR-023 — Optional incoming
**SHOULD:** English Meeting Sound → final English transcript → Indonesian text.

**MUST:** Incoming failure never block otherwise healthy outbound.

### PR-024 — Exclusions
**MUST NOT:** Claim participant identity from mixed/device-level incoming audio.

## 4. First use and readiness

### PR-025 — Minimal setup
**MUST:** First use explain only required concepts: microphone, optional Meeting Sound, TranslateIT Meeting Microphone, selected Meeting voice and local translation readiness.

### PR-026 — Microphone verification
**MUST:** Functionally check an explicit microphone before replacing a known working preference. `Follow Windows Default` preserves default-device intent. A missing pinned mic never silently switches to another.

### PR-027 — Meeting Sound
**MUST:** Meeting Sound represent the Windows output source used by optional incoming. Its failure is scoped to incoming.

### PR-028 — Functional outbound readiness
**MUST:** Readiness verify the actual required local ASR → translation → selected-Meeting-voice synthesis contract, not file presence only.

**MUST NOT:** Missing/failed selected voice silently substitute another engine/voice/cloud path.

### PR-029 — Returning use
**MUST:** Returning users go directly to Meeting with bounded product-level preflight when setup and a selected Meeting voice are available.

## 5. Meeting speech boundary

### PR-030 — Session Listening
**MUST:** One explicitly started continuous Session Listening mode remain the normal capture model.

### PR-031 — Lifecycle
**MUST:** `Ready → Starting → Live → Stopping → Ended` remain the normal lifecycle.

### PR-032 — Final speech truth
**MUST:** Normal translation/TTS/transcript output use finalized stable utterances only.

### PR-033 — Natural segmentation
**MUST:** Segmentation handle natural pauses/boundaries without promoting one historical silence constant into product law.

### PR-034 — Internal tuning
**MUST:** VAD/silence/pre-roll/chunk tuning remain internal.

### PR-035 — Stale-work rejection
**MUST:** Session/generation/utterance identity prevent late old work from producing current output.

### PR-036 — Capture/output coordination
**SHOULD:** Capture continue while a previous finalized utterance is processed when safe. **MUST:** English TTS outputs remain serialized.

### PR-037 — At-most-once delivery
**MUST:** Meeting playback be at-most-once by default; uncertain/partially played output is not blindly replayed from the beginning.

## 6. Translation

### PR-040 — One canonical behavior
**MUST:** Meeting and Text expose one canonical ID ↔ EN translation behavior, not user-selectable model/mode choices.

### PR-041 — Fidelity priority
**MUST:** Preserve intended meaning first, then facts/names/numbers/dates/units/URLs/versions/technical terms, then natural grammar.

### PR-042 — Mixed language
**SHOULD:** Common Indonesian/English code-switching and technical terms remain understandable without a speculative glossary subsystem becoming prerequisite.

### PR-043 — Technical terminology without user bias
**SHOULD:** Preserve established technical terms through the canonical translator. Stored user terminology and Spoken Terms remain inert; they must not enter Text, Meeting or ASR requests.

### PR-044 — Single translation behavior
**MUST:** Use one natural baseline translation behavior. Do not expose Natural/Formal or other user-selectable style instructions.

### PR-044A — No alternative text generation UI
**MUST NOT:** Expose alternative wording or clipboard-to-caption tools on the Text page or normal Quick Actions.

### PR-044B — Truthful output and review
**MUST NOT:** Treat EOS/response framing as proof of semantic fidelity. Known prompt echoes, language mismatches and incomplete text must be rejected rather than promoted to spoken audio or a reusable result. A generic warning is not a replacement for quality validation.

### PR-045 — Context asymmetry
**MUST:** Outbound Meeting translation uses only the current finalized Indonesian utterance. Previously committed translations may not become prompt examples.

**MUST:** Incoming Meeting translation remain context-free.

**MUST:** Text remain standalone and not consume Meeting context.

**MUST NOT:** General History/Saved data become automatic model context.

### PR-046 — Complete or explicit failure
**MUST NOT:** Silently truncate source or promote known incomplete generation. Return bounded explicit failure instead.

### PR-047 — Replaceable implementation
**MAY:** Replace the exact translation model/provider when evidence improves quality/latency/memory/package fit, but the replacement stays inside one canonical pipeline.

## 7. Runtime reliability

### PR-050 — No Realtime/Quality split
**MUST NOT:** Expose Realtime/Quality as user modes.

### PR-051 — Latency philosophy
**SHOULD:** Accept bounded post-speech delay when it materially improves complete stable translation.

### PR-052 — Outbound latency metrics and evidence boundary
**MUST:** Keep `outbound_latency_ms` defined precisely as *finalized utterance → first native translated-audio playback callback*. It is a post-finalization diagnostic metric, **not** the complete delay experienced by the speaker or proof of receipt by another participant.

**SHOULD:** Also measure speech onset/end → first translated audio with traceable audio-boundary timestamps. Existing VAD speech-span and trailing-silence estimates must be explicitly labeled **estimated** until sample-accurate capture timing has been proven.

**MUST NOT:** Fabricate zero latency when first playback is missing, double-count scheduler wait included in ASR/translation/TTS stage totals, or treat local playback as Google Meet delivery proof. Compare P50/P95 and quality only on matched, actually executed scenarios.

### PR-053 — Atomic Start
**MUST:** Commit `Live` only when required outbound path, including selected Meeting voice and route, is actually ready. Duplicate Start must not create another session.

### PR-054 — Resource priority
**MUST:** `Meeting outbound > Meeting incoming > Text > diagnostics/setup`; queues remain bounded. My Voice training is outside this live queue and mutually exclusive with Meeting.

### PR-055 — Bounded recovery
**MUST:** Required outbound recovery remain bounded and owned by the canonical Meeting owner. Newer explicit user action beats stale automatic recovery.

### PR-056 — Stop
**MUST:** Revoke output authority before capture/work cleanup and final ended state.

## 8. Acceleration

### PR-060 — CUDA
**SHOULD:** Use validated CUDA when beneficial.

### PR-061 — GPU scope
**MUST NOT:** NVIDIA GPU be required for app startup or ordinary Text merely because Meeting/My Voice may benefit from it.

### PR-062 — CPU truth
**MUST:** Text remain truthful on supported CPU-only operation when the translation runtime is available.

### PR-063 — Meeting performance truth
**MUST:** Report impractical CPU/GPU Meeting performance honestly rather than claiming equivalent realtime behavior.

### PR-048 — Built-in voice preview
**SHOULD:** Built-in Meeting voices expose a local preview before selection using their packaged approved reference audio. Preview must not require model inference, Meeting start, or voice replacement.

### PR-049 — Meeting setup disclosure
**SHOULD:** When Meeting is fully ready, the normal screen collapses detailed readiness cards into one concise ready summary. Detailed microphone/voice/route states remain visible when setup needs attention and in Settings/Diagnostics.

## 9. Windows audio

### Canonical Meeting audio topology

```text
REQUIRED OUTBOUND
Physical spoken microphone (selected Windows capture endpoint)
  -> TranslateIT mic capture / finalized speech / local ID->EN AI
  -> selected English Meeting voice TTS
  -> Rust playback to CABLE Input (VB-Audio playback endpoint)
  -> paired CABLE Output (VB-Audio recording endpoint)
  -> meeting application's microphone INPUT

OPTIONAL INCOMING (text captions only)
Meeting application's physical speaker/headset OUTPUT
  -> Windows endpoint loopback capture (same selected Meeting Sound endpoint)
  -> finalized English speech / local EN->ID translation
  -> text captions (not an output microphone)

INDEPENDENT SETTINGS
TranslateIT Microphone = spoken physical input to AI
TranslateIT Meeting Sound = optional capture of existing meeting-app speaker output
Meeting-app Microphone = CABLE Output (must be chosen inside the calling app)
Meeting-app Speaker = physical headset/speakers (chosen inside the calling app)
```

Neither TranslateIT's Meeting Sound selector nor its virtual microphone route changes the calling application's speaker setting. VB-CABLE device names are inverted relative to the application: CABLE Input is a Windows playback/render endpoint, and CABLE Output is a Windows recording/capture endpoint. Required and optional audio lanes never exchange raw buffers or become fallback devices for one another. Existing endpoint-wide loopback is not process-only capture and may include other applications' sound.


### PR-070 — Outbound content
**MUST:** Meeting output contain translated English speech from the selected Meeting voice, never raw Indonesian mic audio or a silent fallback voice.

### PR-071 — Physical microphone
**MUST:** Physical microphone remain outbound ASR capture source.

### PR-072 — Meeting Microphone
**MUST:** Provide TranslateIT Meeting Microphone or a functionally equivalent supported Windows endpoint selectable by meeting applications.

### PR-073 — Missing route
**MUST:** Missing route block required outbound Start/delivery with clear Setup Needed recovery; stale queued speech is not dumped after recovery.

### PR-074 — Incoming separation
**MUST:** Incoming use Meeting Sound and remain optional/degradable.

### PR-075 — Own-TTS suppression
**MUST:** TranslateIT's own TTS not become incoming translation. If safe incoming suppression cannot be maintained, degrade/disable incoming rather than required outbound.

### PR-076 — Incoming truth
**MUST NOT:** Invent speaker/participant identity.

### PR-077 — Incoming freshness
**MUST:** Prefer current comprehension over unbounded old subtitle backlog. Automatic mid-session default-device rebind remains outside current scope.

### PR-078 — Audio lane isolation
**MUST:** Meeting Start reject a resolved spoken microphone that is the same Windows recording endpoint carrying the translated virtual Meeting Microphone. Optional incoming capture **MUST NOT** bind to the virtual TTS playback endpoint, including when it is the Windows default speaker. Losing optional incoming must never cancel otherwise healthy outbound translation.

### PR-079 — Display-name collision safety
**MUST:** In the pinned CPAL name-based runtime, audio discovery and stream preparation agree: duplicate endpoint display names cannot be treated as one usable pinned device. Never silently choose the first same-named input, meeting-sound output or virtual playback endpoint. Stable Windows endpoint-ID migration remains a separate native compatibility and dependency decision.

## 10. Text

### PR-090 — Explicit action
**MUST:** Text translate only on explicit Translate.

### PR-091 — Simple workflow
**MUST:** Type/paste → choose direction → Translate → review → Copy.

### PR-092 — Result safety
**MUST:** Older results never overwrite newer user intent; changed source makes prior result visibly previous/outdated.

### PR-093 — Size truth
**MUST NOT:** Silently truncate large Text input; expose a clear bounded limit/failure.

### PR-094 — Text cache
**MAY:** Reuse identical Text translations from a small in-memory per-launch cache keyed only by source and translation direction. Cache must be invalidated on translation-contract changes.

**MUST:** Cache remain bounded and process-local. **MUST NOT:** Persist translation results across app launches or use cached Meeting context.

### PR-095 — Quick Translate
**MUST NOT:** Expose clipboard-to-caption Quick Translate. No retired Quick Translate or alternative-wording native commands may remain registered.

**MUST NOT:** Run a background clipboard watcher.

## 11. Privacy and storage

### PR-100 — No general persistence dependency
**MUST NOT:** General History/Saved persistence be required for Meeting/Text.

### PR-101 — Temporary data
**DEFAULT:** Raw mic/Meeting Sound/generated Meeting TTS/live transcripts and unfinished My Voice review drafts/build intermediates are temporary. My Voice **explicitly accepted** guided takes are separately retained locally for future improvement under PR-106; this exception never turns Meeting audio or transcripts into durable history.

### PR-102 — Diagnostics privacy
**MUST:** Diagnostics remain minimal/redacted; no full conversation/raw audio logging by default.

### PR-103 — Storage roots
**MUST:** Preserve `CacheData` as temporary workspace and legacy current-take compatibility, `LogData` redacted diagnostics, and `SavedProject/VoiceLab` as the existing approved actor location plus protected copies of explicitly accepted My Voice recordings. `VoiceLab` is storage compatibility vocabulary only. Do not delete legacy takes during migration.

### PR-104 — Temporary session review
**MAY:** Retain only the most recently ended Meeting transcript in memory for explicit review/copy/export until a new Meeting starts or the app exits.

**MUST NOT:** Turn this into general persistent History.

**MUST:** Each new Meeting starts with a unique session identity; the next Meeting must not append to the last Meeting's transcript. The last unexported transcript is replaced when the next session starts or the app closes.

**MUST:** Export remains explicitly user-triggered. Saved transcript files live under separate session-scoped folders; no automatic permanent save. Live and Last Meeting views are limited to 240 recent phrases, while full-session manual export retains up to 4,096 turns or 8 MiB of source/translation text in RAM (whichever bound is reached first). Any actual evictions must be indicated honestly as incomplete. No unlimited archive or automatic disk spool is implied.

**SHOULD:** Future full-session capture, if approved, use a bounded temporary store with explicit cleanup and privacy; do not silently convert manual export into persistent background recording.

**MUST:** Record the immutable recognized source turn before translation begins. Keep `translation_state` independent of outbound `delivery_state`. On translation failure, retain the source and show an explicit unavailable state; an empty result is never a successful translation.

**MUST:** Reject late translation completion after Stop or a session/generation change. Deferred incoming translation that expires or is evicted must mark an already-recognized source as failed without affecting outbound. Pending turns may not remain "Translating" after Meeting ends.

### PR-105 — Translation feedback
**MAY:** Store opt-in local translation issue reports for quality review.

**MUST:** Feedback remain bounded, explicitly reviewable/exportable/clearable, and never become automatic model context.

### PR-106 — Durable accepted My Voice recordings
**MUST:** Once the user explicitly accepts a guided take, preserve an immutable, verified on-device copy tied to the exact guided line and transcript before replacing an older active take. A new take of the same line must not delete a prior saved version. A failed archive verification must be reported as an unsuccessful save, not hidden or presented as protected.

**MUST:** Legacy recording migration only copies and verifies recognized source files, including interrupted `.wav.previous` recovery backups when present. Never delete, rename or silently reinterpret originals during migration. Unknown, corrupt, unpaired or unverified files remain visible as attention, not claimed protected.

**MUST:** Keep durable user recordings distinct from temporary training work, Meeting audio and transcripts. No automatic network upload, automatic garbage collection of accepted takes, or voice-audio diagnostics logging. Backups/export and user-directed deletion require their own explicit, verified interfaces before being described as available.

### PR-107 — Immutable My Voice datasets and voice versions
**MUST:** Before starting a new My Voice training worker, preserve a content-addressed manifest of the exact frozen guided dataset and verify that every referenced canonical recording and exact guided transcript exist in the permanent accepted-take archive. Training continues to use the pinned canonical WAV and dataset format; the persistent manifest is provenance, not a new training engine.

**MUST:** Before approved actor promotion or replacement of a validated personal voice, preserve its full inference package (GPT weights, SoVITS weights, reference WAV and actor manifest) as an immutable, hash-verified version. Version identity must be independent of mutable selected-voice paths; interrupted or corrupt staging must not be silently deleted or called ready.

**MUST:** Tie each newly approved candidate's durable actor package to its frozen dataset snapshot where available, without claiming legacy training lineage when it is unknown. A retained inference actor is **not** a resumable training checkpoint.

**MUST NOT:** Activate a stored historic actor, delete a historic actor, change the selected Meeting voice, or claim successful future-training continuation merely because an immutable registry record exists. Selection/restore UI and resumable optimizer checkpoints require separate explicit guards, user approval, and end-to-end verification.

### PR-108 — Saved personal voice version selection
**MUST:** List earlier immutable personal actor versions as candidate saved entries without claiming full model integrity merely from listing metadata. Verify exact actor manifest, reference, GPT and SoVITS weight hashes before selecting any historic version.

**MUST:** A historic voice switch uses the same Meeting/recording/training resource authority and selected-actor compatibility path as normal voice selection. Require explicit confirmation before changing the current Meeting voice; do not change it during Meeting, microphone recording, training, or Quick Preview. Preserve the previous personal actor first, use guarded atomic replacement, and invalidate outbound readiness after successful selection.

**MUST:** Incomplete prior swap transactions, invalid actor packages, wrong file hashes, or uncertain command responses block additional unsafe changes. The UI must clearly distinguish verified playback/selection from a catalog entry that has only passed metadata checks.

**MUST NOT:** Automatically delete older versions or imply an inference actor is a full optimizer resume checkpoint. Multi-version comparison, full backup/export, training checkpoint resume and off-device durability are separately scoped work.

### PR-109 — Improve My Voice using earlier accepted recordings
**MUST:** Rebuild from user-selected immutable, hash-verified accepted recordings using the existing pinned GPT-SoVITS V2ProPlus pretrained training pipeline. Only one recording per known, unchanged guided English prompt may be used in a run; selection must preserve exact prompt ID and transcript, enforce current bounded worker take count, speech duration, category variety and usable reference. No arbitrary ASR transcription changes.

**MUST:** Freeze user-selected archive bytes into a new temporary GPT-SoVITS dataset and preserve a permanent snapshot tying each training take to its exact archive identity. A newer accepted take must never silently replace the frozen audio used for training, and must not invalidate a completed archive-backed held-out review. Corrupt, mismatched or missing snapshots/recordings block candidate review and approval.

**MUST:** Keep every prior accepted recording and previously approved voice version intact while training or evaluating the new candidate. Activate only after the existing eight held-out sample checks and explicit human listening/quality approval. Failure, cancellation or rejection must not replace the active Meeting actor.

**MUST NOT:** Label rebuilding with selected archived takes as warm-start training, optimizer checkpoint resume or proven quality improvement. Full 600+ prompt expansion and run capacities beyond the current validated bound are separate work.

## 12. My Voice and built-in voices

### PR-110 — My Voice workflow
**MUST:** authorized recording → replay/accept/retry → dataset → train → held-out evaluate → user preview/approve → save.

### PR-111 — Authorization
**MUST:** Require confirmation that custom training uses the user's voice or an authorized voice.

### PR-112 — Guided source truth
**MUST:** Application-provided English lines supply exact transcripts; do not add ASR merely to label known guided recordings.

### PR-113 — Quality-controlled takes
**MUST:** User can replay/accept/retry and build-time checks reject unusable takes.

### PR-114 — Quality-first training
**MUST:** My Voice fine-tune GPT-SoVITS V2ProPlus; no retraining on app/Meeting/utterance start.

### PR-115 — No arbitrary quality constant
**MUST NOT:** Treat recording minutes, wall-clock training, epochs or similarity score alone as proof of a good actor.

### PR-116 — Held-out evaluation
**MUST:** Evaluate unseen English lines and require user listening/approval before promotion.

### PR-117 — Native baseline
**MUST:** Establish accepted native engine quality before optional export/quantization optimization.

### PR-118 — Atomic rebuild promotion
**MUST:** Keep previous approved actor until a new build completes evaluation and explicit approval.

### PR-119 — Selected Meeting voice
**MUST:** Meeting use one selected voice contract through the canonical local runtime: either a packaged built-in voice or an approved My Voice actor. Start must obtain bounded functional selected-voice readiness before `Live`.

**MUST NOT:** Run My Voice training concurrently with active Meeting.

**MUST NOT:** Change the selected Meeting voice during a live/starting/stopping Meeting, including approving a newly evaluated My Voice actor. The native approval transaction must reject the change without replacing the current actor; the UI must show a recoverable Stop Meeting instruction.

### PR-120 — My Voice extensible guided corpus
**MUST:** Preserve the exact authored English sentence and stable numeric identity of guided training lines 1–128; accepted recordings, migration and dataset snapshots must never silently change or relabel their original transcriptions. Training-only prompt IDs 129–600 remain separate from reserved held-out IDs 1001–1008.

**MUST:** Provide a user-selectable, browseable library of 600 authored English prompts with clear presentation levels: **Short Conversations**, **Natural Conversations**, and **Detailed Explanations**. Reading level is a navigation aid, not a model-quality grade, accent score, or mandatory completion target. Each prompt retains an independent internal focus tag and one of five legacy-compatible recording-coverage groups.

**MUST:** Guided recording, saved recording identifiers, training selection, and migration share the same Rust-owned prompt text and metadata. Offer search, category/focus filters, and bounded pagination; do not place all 600 prompt controls on screen simultaneously or demand every line to start training. Preserve the pinned 128-take worker cap per build; beyond it, explain that the user must select a bounded subset of saved takes rather than begin a run that the worker will reject.

**MUST:** Suggest missing training categories using semantic metadata, not numeric legacy ranges, and preserve the legacy five-group eligibility behavior for all existing guided IDs. Do not conflate the size of a prompt library with acoustic, phoneme, or voice-model quality guarantees. Optional pronunciation review must not replace application-authored transcript text.

### PR-121 — Advisory sound-pattern recording guidance
**MUST:** Recommend optional English training lines based on documented, deterministic *text-derived* sound-pattern coverage from the immutable authored prompt corpus and the exact set of accepted guided line IDs. Recommendations may prioritize unrepresented training coverage groups, estimated pronunciation symbols, selected within-word sound transitions, varied sentence focus and beginner-readable prompts. Neither suggestions nor estimated phone/transition coverage may become a new training eligibility gate.

**MUST:** Bind generated pronunciation metadata to exact prompt text and ID. A missing, mismatched or stale inventory must fail closed to no phonetic recommendation, not falsely claim that a line adds acoustic variation. Keep the existing five coverage requirements, one-minute minimum, reference duration and current 128-take per-build limit unchanged.

**MUST:** Explain suggestions as *estimated from written English* rather than verified pronunciation, a percentage of model quality, a required number of recordings or a judgment of non-US accents. Dictionary-unknown words are uncertain, not incorrect speech. Users may ignore suggestions, choose another line, or stop recording without losing accepted takes.

**MUST NOT:** Run a new ASR, G2P or voice model automatically on every UI status refresh; upload raw audio or transcripts for recommendations; install a second TTS engine; overwrite existing exact guided texts. Generate compact metadata in developer tooling from a reviewed pronunciation source; do not bundle an unrelated cloud API or runtime dictionary merely to display suggestions.

### PR-122 — Explicit portable My Voice backup and additive restore
**MUST:** Offer user-initiated local/offline backup creation to an existing chosen destination directory, in a new unique child folder. Include the durable, immutable Recording Library, DatasetSnapshots, TrainingLineage, and VoiceVersions; first preserve any current or separately saved personal actor and protect recognized legacy accepted takes. Do not include temporary Meeting recordings, Meeting transcripts, diagnostics, personal credentials, cache-only build intermediates, or unrelated files.

**MUST:** Use a portable versioned manifest with safe relative filenames, bounded file sizes/counts, SHA-256 of copied bytes, complete reference checks, and a committed/verified export folder. Unknown, missing, corrupt, unrecognized, symlinked or unfinished archive objects prevent a misleading success notice. Keep failed partial staging for explicit recovery rather than automatically deleting user data.

**MUST:** Support read-only backup verification separately from restore. Restore may only merge immutable archived objects after complete source verification and an explicit authorized-voice confirmation. Existing identical objects remain untouched; conflicting existing objects cause a failure before import writes. Never overwrite the current Meeting voice, select an imported model automatically, silently remove old versions, or erase original recordings.

**MUST:** Label integrity verification accurately: a self-contained SHA-256 manifest does not prove authenticity of an untrusted backup. Explain that a backup on the same disk does not protect against disk failure; restoration and voice usability on another Windows device still require target-Windows validation. The native desktop may initially require copying an absolute destination/folder path while a later native folder picker is evaluated without a second storage authority.

**MUST NOT:** Automatically sync My Voice content to a remote/cloud service. Never claim device-level recovery or backup durability before a real cross-path restore has been executed and verified on the target platform.

### PR-123 — Preserve interrupted training checkpoints without unsafe resume
**MUST:** Before starting a new My Voice training build, and after an unsuccessful/cancelled training child exits, inspect the pinned GPT-SoVITS V2ProPlus work area for full training checkpoint candidates, including GPT Lightning \`ckpt\` and SoVITS Generator/Discriminator optimizer-containing checkpoints. Preserve any found checkpoint bytes and relevant training config in an immutable, SHA-256-verified local recovery archive **bound to the original frozen dataset snapshot**. If preservation cannot be verified, do not clear the old work area or silently reuse it with a different dataset.

**MUST:** Keep checkpoint recovery objects logically separate from approved inference Voice Versions, accepted Recording Library data and dataset snapshot records. Include successfully preserved checkpoint archives and their dataset references in explicit portable My Voice backups. Any incomplete or mismatched recovery capture remains visible as attention; do not delete it or mislabel it as an approved voice.

**MUST:** Label saved training recovery bundles as **checkpoint candidates, resume unverified** until both GPT and SoVITS states have been structurally and operationally validated against the exact pinned trainer, config, dataset, engine revision and GPU/precision environment. A file existing or a checkpoint-loading attempt must not be presented as evidence that optimizer-based resume happened.

**MUST NOT:** Automatically restart optimizer training, use only inference weights as a resume checkpoint, clear abandoned optimizer state on an unverifiable source, or claim a full bit-for-bit resume with the existing upstream SoVITS implementation. Upstream SoVITS can broadly catch checkpoint loading failures and fall back to pretrained weights, reconstructs its schedulers, and does not show an AMP GradScaler state in its full checkpoint saving path; these limitations require explicit fail-closed instrumentation and target-Windows acceptance before a user-facing Resume action is authorized.

### PR-124 — Strict read-only training-resume admission
**MUST:** Separate \`checkpoint preserved\`, \`checkpoint integrity verified\`, \`training state structurally inspected\`, and \`resume execution verified\`. A saved GPT/SoVITS model or successful hash check does not establish that optimizer, scheduler, global step and automatic mixed-precision state were restored.

**MUST:** Provide a developer-only, opt-in and read-only audit of a specified immutable \`TrainingRecovery/<id>\` archive. Verify archive ID and bounded SHA-256 of all referenced files, frozen dataset snapshot, exact guided take/transcript identities and the pinned GPT-SoVITS upstream source. Check paired SoVITS G/D checkpoint filenames and config identities. When tensor-state inspection is explicitly requested on a trusted local archive, attempt only restricted \`torch.load(..., weights_only=True)\`; never retry with unsafe deserialization to produce a green status.

**MUST:** Explicitly report incomplete GPT Lightning optimizer/scheduler state, SoVITS generator/discriminator mismatch, unusable precision config and missing evidence of the upstream training-loader continuation. An uninspectable tensor checkpoint must remain an uncertainty, not be falsely labelled corrupt or resumable.

**MUST NOT:** Automatically launch a resumed build, modify the pinned engine, overwrite a checkpoint or selected voice, delete unvalidated recovery files, or expose an enabled Resume control until both trainer load acknowledgment and target-Windows epoch/optimizer continuity are proven on the exact frozen dataset. The upstream SoVITS fallback-to-pretrained and missing AMP GradScaler state remain blockers.

### PR-125 — Strict worker-origin checkpoint load acknowledgment
**MUST:** A future opt-in resume execution must prove both (a) exact immutable checkpoint and dataset identity, and (b) actual load of model/optimizer states inside the trainer worker that continues training. Pinned SoVITS \`torch.multiprocessing.spawn\` means parent-only instrumentation does **not** establish what was loaded by spawned children.

**MUST:** Bind acknowledgment events to one run token, recovery archive ID, dataset snapshot ID, pinned engine revision, expected checkpoint SHA-256 for GPT/SoVITS Generator/Discriminator, training epochs and worker process identity. A protocol validator must reject missing, duplicate, reordered, conflicting, wrong-process, wrong-checkpoint and false optimizer/scheduler acknowledgments. A reported pretrained-weight fallback irrevocably invalidates the attempt.

**MUST:** Require separate evidence for worker-side checkpoint loading and first resumed training-step state. SoVITS Generator/Discriminator must be acknowledged by the same training worker at the same saved epoch; GPT must report a consistent checkpoint epoch, global step and scheduler state. A JSONL log or successful subprocess exit is *not* itself authenticated proof of worker origin. An apparently complete sequence must not be called \`resume_verified\` until native worker instrumentation and cross-process provenance have been validated.

**MUST NOT:** Automatically resume training, modify selected Meeting voice, touch accepted takes/immutable archives, intercept unrelated process state, or expose a working Resume button merely because the event-contract parser exists. The existing SoVITS broad pretrained fallback, reconstructed scheduler, and absent AMP GradScaler checkpoint remain blocking acceptance conditions. Pinned GPT-SoVITS model/engine source must remain unchanged.

### PR-126 — Opt-in spawned SoVITS child-origin resume probe
**MUST:** An explicitly invoked developer-only SoVITS checkpoint probe may replace only the pinned trainer's checkpoint loader boundary inside the actual \`torch.multiprocessing.spawn\` child. The producer must have an importable module entry point on Windows and verify the pinned upstream trainer files and source path in both launcher and child.

**MUST:** Before reporting SoVITS Generator/Discriminator checkpoint loaded, verify the exact planned paths and SHA-256, safeload trusted checkpoint data using \`weights_only=True\`, require matching complete model state keys/shapes and nonempty optimizer state, and confirm both paired checkpoints use the same saved epoch. A mismatch must hard-fail the probe child BEFORE the pinned \`s2_train.py\` broad \`except\` can fall back to pretrained weights. Do not treat a successful model-only weight load as optimizer-resume proof.

**MUST:** Emit the initial SoVITS training-step event only after the first real Generator \`optimizer.step()\` call completes at the expected next epoch, not merely when the epoch starts (AMP may skip the step). Bind emitted evidence to run/recovery/dataset/checkpoint identities and child PID. Treat a writable JSONL file as diagnostic evidence, **not authenticated proof of worker origin**; the separate fail-closed acknowledgment protocol must continue returning \`resume_available=false\` until independent native cross-process provenance, GPT Lightning restoration and scheduler/GradScaler continuity have been established.

**MUST NOT:** Route ordinary My Voice creation or Improve My Voice through this probe. An explicit \`--translateit-dev-strict-sovits-probe <plan>\` flag is mandatory and disabled on all ordinary production command paths. Do not modify the pinned GPT-SoVITS source or models, auto-resume any checkpoint, change the selected Meeting voice, or expose the end-user Resume action.

### PR-127 — Developer-only GPT Lightning worker restore probe
**MUST:** GPT resume diagnostics may instrument the pinned \`GPT_SoVITS/s1_train.py\` only behind a separately named, explicit developer CLI flag. Ordinary \`Create My Voice\`, \`Improve My Voice\`, inference, and Meeting must not invoke this probe or change the pretrained engine.

**MUST:** Before calling Lightning \`Trainer.fit\`, require a matching explicit full checkpoint, SHA-256, training config SHA-256, frozen dataset/recovery/run identities, expected epoch and global step. A missing checkpoint must hard-fail the developer probe rather than silently trigger fresh training. Verify the exact pinned GPT-SoVITS trainer source before installing any probe code.

**MUST:** Observe the real Lightning training lifecycle after restoration; compare the active model, optimizer and scheduler states to the full checkpoint in a bounded, strict way, verify restored epoch and global step, then emit an initial checkpoint-load diagnostic event. Emit a separate first-training-step event only after an actual optimizer step followed by an increment of Lightning's \`global_step\`. A skipped AMP optimizer step, a failure to restore state, mismatched scheduler, or a completed run without verified progress cannot be acknowledged as resume success.

**MUST:** Keep developer event logs diagnostic and explicitly untrusted as standalone evidence. Even when both GPT and SoVITS traces are syntactically complete, the existing strict acknowledgment validator must retain \`worker_provenance_verified=false\`, \`trainer_load_verified=false\`, and \`resume_available=false\` until cross-process provenance, AMP/scheduler continuity and target-Windows GPU execution have been demonstrated.

**MUST NOT:** Introduce a second TTS engine or replacement model, patch upstream GPT-SoVITS files on disk, automatically select a different personal voice, delete training checkpoints, use unsafe checkpoint deserialization as a fallback, or expose an active end-user Resume button.

## 13. UI

### PR-160 — Navigation
**MUST:** Meeting | Text | My Voice | Settings.

### PR-161 — Default workspace
**MUST:** Meeting is default workspace.

### PR-162 — Meeting Ready
**MUST:** Show only one primary Meeting control: Start when ready, Stop while active, or an appropriate single recovery action when blocked. Show finalized chronological conversation during Live. Do not expose device selectors or Meeting presets on the main Meeting page.

### PR-163 — Meeting Live
**MUST:** Emphasize Translation Live, finalized chronological transcript, simple activity and Stop Translation.

**MUST:** Present the conversation as chat bubbles: `You` on the right and `Person 1` on the left, using one shared presentation for Live and Last Meeting review. Show original language/text, translation language/text, and recorded timestamp.

**MUST NOT:** Claim that `Person 1` identifies one actual participant. It labels the combined incoming Windows output audio until speaker diarization exists and is validated. Do not invent `Person 2` or `Person 3` based on mixed audio.

**MUST:** Display the session's date/time rather than an ungrounded global Meeting 1, 2, 3 counter; manual export folders must preserve distinct session identities.

### PR-164 — Settings
**MUST:** Use left navigation for Voice & Audio, Meeting Captions and Help; retain device selection/probe only under Settings. My Voice creation remains on My Voice. Technical diagnostics belong in Help.

### PR-164A — Meeting Presets retired
**MUST NOT:** Offer, suggest, automatically apply or mutate Meeting Presets in the active product.

**MUST:** No legacy preset-apply command may remain registered or callable. Existing local preference data must not be erased automatically.

### PR-165 — Vocabulary
**MUST:** Normal users see product states such as Ready, Live, Setup Needed, Unavailable, Checking, Training, Needs Review rather than internal runtime vocabulary.

### PR-166 — Familiar interaction
**MUST:** Keep direct source/target/action/result relationships and calm healthy states; emphasize warning/recovery only when action is required.

## 14. Lifecycle

### PR-176 — Long-session bounds
**MUST:** Memory, queues, temp artifacts, threads and handles remain bounded.

### PR-177 — Minimize
**MUST:** Minimize/hide does not stop a healthy Meeting.

### PR-178 — Safe close
**MUST:** Closing active Meeting uses canonical Stop before destruction.

### PR-179 — Sleep/hibernate
**MUST:** Power transition invalidates active output authority; no automatic old voice replay after wake.

## 15. Packaging

### PR-140 — Distribution
**MUST:** Controlled Windows users/owned machines are the current distribution boundary.

### PR-141 — One setup experience
**MUST:** One user-facing Setup owns install/repair flow.

### PR-142 — No developer setup for users
**MUST NOT:** Installed users manually operate Python/pip/env vars/repository checkout/core-model placement/GPT-SoVITS tooling.

### PR-143 — Controlled packaged assets
**MUST:** Release inputs provide required helper/runtime, ASR, bidirectional translation, selected-voice inference assets, My Voice build assets and Meeting route support. Built-in LibriSpeech/OpenSLR references retain CC-BY-4.0 source/attribution metadata.

## 16. Evidence / release gate

### PR-180 — Source is not live proof
**MUST:** Static source/config never be reported as target runtime/device/model success when that context did not run.

### PR-181 — Acceptance boundary
Target-capable acceptance remains scenario-specific and includes as applicable: physical microphone capture, final ASR, ID→EN and EN→ID translation, built-in voice Meeting use, My Voice train/evaluate/reuse, Meeting Microphone delivery, optional incoming behavior, Stop/Close, end-to-end latency/stability, standalone Text, install/runtime and clean-machine behavior.

No arbitrary quality/latency threshold is invented without relevant evidence.
