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

**NOT CURRENT CORE:** Document Translation, general History/Saved UI, Audio Studio, Push to Talk, Pause/Resume, Realtime/Quality user modes, tone selectors, additional languages, partial translated subtitles, incoming Indonesian TTS, imported-audio/quick-clone My Voice, multiple normal voice engines/providers.

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

### PR-043 — Technical terminology
**SHOULD:** Preserve established technical terms when translating them reduces clarity.

**MUST:** Support a small user-managed Indonesian ↔ English terminology list for names, product terms, and recurring technical wording. Only entries relevant to the current source text enter the model prompt, direction reverses automatically for EN→ID, and terminology remains bounded so it cannot displace the current utterance or required Meeting context.

**MUST NOT:** Implement terminology as blind post-translation search/replace.

### PR-044 — No tone selector
**NOT CURRENT CORE:** Auto/Formal/Casual user controls.

### PR-045 — Context asymmetry
**MUST:** Outbound Meeting translation may use only the current finalized Indonesian utterance plus up to the last three committed own-voice ID→EN pairs from the same live session.

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

### PR-052 — Outbound latency metric
**MUST:** Measure user-relevant outbound latency from finalized utterance end to first translated audio playback when evaluating live performance.

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

## 9. Windows audio

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

## 10. Text

### PR-090 — Explicit action
**MUST:** Text translate only on explicit Translate.

### PR-091 — Simple workflow
**MUST:** Type/paste → choose direction → Translate → review → Copy.

### PR-092 — Result safety
**MUST:** Older results never overwrite newer user intent; changed source makes prior result visibly previous/outdated.

### PR-093 — Size truth
**MUST NOT:** Silently truncate large Text input; expose a clear bounded limit/failure.

## 11. Privacy and storage

### PR-100 — No general persistence dependency
**MUST NOT:** General History/Saved persistence be required for Meeting/Text.

### PR-101 — Temporary data
**DEFAULT:** Raw mic/Meeting Sound/generated Meeting TTS/live transcripts and My Voice build intermediates are temporary until a resulting actor is explicitly approved.

### PR-102 — Diagnostics privacy
**MUST:** Diagnostics remain minimal/redacted; no full conversation/raw audio logging by default.

### PR-103 — Storage roots
**MUST:** Preserve `CacheData` temporary, `LogData` redacted diagnostics, `SavedProject/VoiceLab` approved persistent My Voice actor. `VoiceLab` is storage compatibility vocabulary only.

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

## 13. UI

### PR-160 — Navigation
**MUST:** Meeting | Text | My Voice | Settings.

### PR-161 — Default workspace
**MUST:** Meeting is default workspace.

### PR-162 — Meeting Ready
**MUST:** Emphasize required devices, selected Meeting voice, language direction and Start Translation; hide engine/model/queue internals from normal users.

### PR-163 — Meeting Live
**MUST:** Emphasize Translation Live, finalized chronological transcript, simple activity and Stop Translation.

### PR-164 — Settings
**MUST:** Meeting settings own device/setup preferences; Advanced owns diagnostics. My Voice creation remains on My Voice.

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
