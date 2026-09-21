# TranslateIT Context

## Repository authority

**Local-only repository model**

```text
Local = sole active working/development/governance/CI/proof authority
```

Routine repository work is performed on `Local` only. Other branches are outside the current development lifecycle and are not fallback source, proof, continuation, or promotion targets unless the user explicitly changes this policy.

## Product

TranslateIT is a local-first Windows desktop application for simple Indonesian ↔ English translation, primarily for online meetings.

Current normal navigation:

```text
Meeting
Text
My Voice
Settings
```

Required outbound Meeting flow:

```text
Indonesian speech
→ finalized Indonesian utterance
→ Indonesian → English translation
→ selected Meeting voice
   ├─ Built-in Male/Female by default
   └─ approved My Voice when the user chooses it
→ TranslateIT Meeting Microphone
→ meeting application
```

Optional incoming assistance:

```text
English Meeting Sound
→ finalized English utterance
→ English → Indonesian translation
→ local text only
```

Incoming failure must not block otherwise healthy outbound translation.

## Current product locks

- Windows only for the current product boundary.
- Indonesian ↔ English only.
- One canonical translation pipeline; no user-facing Realtime/Quality modes. Translation wording style is bounded to Natural (default) or Formal.
- Outbound Meeting translation may use rolling context from the last three committed own-voice translation pairs in the same live session.
- Incoming translation remains context-free.
- Text is paste/type only; document/file translation is removed.
- Full English UI copy.
- Session Listening is the normal Meeting capture mode; Push to Talk and Pause/Resume are outside current scope.
- Two ready built-in English voices allow day-one Meeting use without My Voice training.
- My Voice is an optional trained GPT-SoVITS V2ProPlus upgrade created from authorized user recordings.
- No silent cloud fallback or parallel normal AI engine.
- App updates use a signed one-shot startup check: no updater daemon, polling timer, tray service, or forced restart. Release signing and GitHub Release publication remain explicit release operations.

## Current architecture

```text
Tauri 2 desktop application
├─ Svelte 5 + Vite + TypeScript frontend
├─ Rust desktop/runtime backend
└─ one canonical Python local worker
   ├─ ASR
   ├─ ID ↔ EN translation
   └─ GPT-SoVITS voice inference / My Voice build support
```

Current source roots:

```text
EngineData/Frontend/RustApp/
EngineData/Backend/LocalWorker/WorkerRuntime/
EngineData/Backend/RuntimeAssets/
UserData/
```

The Svelte frontend is current architecture, not a future migration target.

## Storage

```text
UserData/CacheData/             temporary runtime/build data
UserData/LogData/               minimal/redacted diagnostics
UserData/SavedProject/VoiceLab/ approved persistent My Voice actor
```

`VoiceLab` in storage/protocol identifiers is a retained compatibility name; product/UI terminology is **My Voice**.

## Release shape

Current packaging direction is one user-facing offline Setup plus one colocated controlled payload:

```text
TranslateIT-Setup.exe
TranslateIT-Payload.7z
```

Release-source validation runs from current `Local`. Normal users do not install Python, run pip, download core models manually, or operate GPT-SoVITS tooling.

## Proof boundary

```text
REMOTE_GITHUB
→ source/static/CI contracts on Local

LOCAL_CODE
→ exact Local checkout/toolchain/build/generator/filesystem proof

TARGET_WINDOWS
→ installed app + real GPU/audio/devices/VB-CABLE/meeting/latency/clean-machine proof
```

Source presence never upgrades itself into runtime or target-Windows proof.

## Canonical navigation

- GitHub mechanics: `GITHUB_RULES.md`
- Agent modes/routing: `AGENTS.md`
- Current product law: `docs/foundation/`
- Continuation: `docs/knowledge/next-action.md`
- Proof interpretation: `docs/knowledge/current-validation.md`
- Ownership: `docs/knowledge/source-ownership.md`
- Decisions: `docs/knowledge/decisions/`
- Flow: `docs/knowledge/flow.md`
- Skills: `docs/knowledge/skills/`
