# TranslateIT — Product Overview

**Status:** Active Policy  
**Updated:** 2026-09-07

## Purpose

TranslateIT is a Windows desktop application for simple, reliable Indonesian ↔ English translation, primarily for online meetings. Translation completion and safe delivery matter more than feature breadth.

## Core product

### Meeting — required outbound

```text
Indonesian speech
→ finalized Indonesian utterance
→ Indonesian → English translation
→ selected Meeting voice
   ├─ Built-in Male/Female by default
   └─ approved My Voice when selected
→ TranslateIT Meeting Microphone
→ meeting application
```

Two built-in English voices allow day-one use without training. My Voice is an optional high-fidelity upgrade created from the user's authorized recordings.

### Meeting — optional incoming

```text
English Meeting Sound
→ finalized English utterance
→ English → Indonesian translation
→ local translated text
```

Incoming is optional/degradable and must never block otherwise healthy outbound translation.

### Text

```text
Indonesian text ↔ English text
```

Text is standalone type/paste translation. It does not depend on Meeting audio or My Voice.

### My Voice

```text
authorized user voice
→ guided English recordings
→ accept/retry takes
→ quality-controlled dataset
→ GPT-SoVITS V2ProPlus training
→ held-out evaluation
→ user preview/approval
→ reusable My Voice actor
```

Training is occasional. Daily Meeting use performs inference only.

## Product locks

```text
Platform        → Windows
Languages       → Indonesian + English only
Translation     → one canonical pipeline
Meeting capture → Session Listening
Meeting voice   → Built-in Male/Female or approved My Voice
Incoming        → optional EN speech → ID text
Text            → paste/type ID ↔ EN
UI language     → English
Runtime         → local-first after required assets are installed
Distribution    → personal/owned machines
```

Removed/deferred from current product:

- Push to Talk;
- Pause/Resume;
- user-facing Realtime/Quality translation modes;
- Auto/Casual tone controls; the only current wording styles are Natural (default) and Formal;
- Document/file translation;
- general History/Saved workspace;
- Audio Studio/broadcast workflows;
- additional language pairs;
- imported-audio/quick-clone My Voice modes;
- multiple normal custom-voice engines/providers;
- partial translated subtitles;
- incoming Indonesian TTS;
- automatic cloud fallback;
- signing/auto-update as current blockers.

## Translation context

Normal translation uses one canonical ID ↔ EN pipeline.

```text
Meeting outbound
→ current finalized Indonesian utterance
+ up to last 3 committed own-voice ID→EN pairs from same live session

Meeting incoming
→ current finalized English utterance only

Text
→ current text only
```

History/Saved data and incoming participant turns never become automatic translation context; incoming remains context-free.

Translation priorities:

```text
1. intended meaning
2. names/numbers/dates/units/URLs/versions/technical facts
3. understandable natural target-language grammar
4. no silent incomplete output
```

## Voice principle

Built-in voices and My Voice feed one selected-Meeting-voice contract. There is no normal user-facing voice-engine/provider selector and no silent fallback to another voice when the selected path cannot synthesize safely.

My Voice remains quality-first trained GPT-SoVITS V2ProPlus. Built-in references use controlled packaged reference material and shared approved inference assets without requiring user training.

## Current application architecture

```text
Tauri 2 desktop application
├─ Svelte 5 + Vite + TypeScript frontend
├─ Rust desktop/runtime backend
└─ one canonical Python local worker
```

Svelte 5 is current architecture, not a future migration target.

## Meeting runtime

Normal lifecycle:

```text
Ready → Starting → Live → Stopping → Ended
```

Only finalized stable utterances become normal translation/TTS/transcript output. Capture may continue while a previous utterance is translated/spoken when safe; translated TTS output remains serialized.

Priority:

```text
Meeting outbound
> Meeting incoming
> Text
> diagnostics/setup
```

My Voice training is mutually exclusive with an active Meeting rather than entering the live scheduler.

## Stop / close

`Stop Translation` revokes current output authority before capture/work cleanup. Normal minimize does not stop a healthy Meeting. Window close while active uses the canonical Stop path first.

## Privacy / storage

```text
UserData/CacheData             temporary runtime/build data
UserData/LogData               minimal/redacted diagnostics
UserData/SavedProject/VoiceLab approved persistent My Voice actor
```

Raw mic/Meeting Sound/generated Meeting TTS/live transcript bodies are temporary by default. My Voice recordings/build intermediates remain temporary until explicit actor approval. Rebuild must not destroy the previous approved actor before the new one is approved.

## UI direction

Normal top-level navigation:

```text
Meeting
Text
My Voice
Settings
```

Normal UI exposes product states/actions, not Python/model/CUDA/VAD/queue/checkpoint internals. Technical details belong in Advanced/Diagnostics.

## Proof standard

Source presence is not runtime proof. Product claims require matching evidence:

```text
source/routing → repository/source checks
compile/build  → actual toolchain/build
model/GPU      → matching runtime/hardware
mic/device     → target Windows
installer      → actual install/runtime
clean machine  → clean target evidence
```

See `02-product-requirements.md` and `03-acceptance-scenarios.md`.
