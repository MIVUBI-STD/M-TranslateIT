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
**DEFAULT:** Raw mic/Meeting Sound/generated Meeting TTS/live transcripts and My Voice build intermediates are temporary until a resulting actor is explicitly approved.

### PR-102 — Diagnostics privacy
**MUST:** Diagnostics remain minimal/redacted; no full conversation/raw audio logging by default.

### PR-103 — Storage roots
**MUST:** Preserve `CacheData` temporary, `LogData` redacted diagnostics, `SavedProject/VoiceLab` approved persistent My Voice actor. `VoiceLab` is storage compatibility vocabulary only.

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
