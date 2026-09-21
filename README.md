# TranslateIT

**Local-first Windows desktop translation for Indonesian ↔ English meetings.**

> **Local-only repository model:** `Local` is the sole active source, development, governance, CI, proof, and continuation authority. Source/CI presence is not proof of target-Windows runtime, GPU, audio-route, installer, or clean-machine behavior.

## Product

```text
Meeting
Text
My Voice
Settings
```

### Meeting

Required outbound:

```text
Indonesian speech
→ final Indonesian transcript
→ English translation
→ selected Meeting voice
   ├─ Built-in Male/Female
   └─ approved My Voice
→ TranslateIT Meeting Microphone
→ meeting application
```

Optional incoming assistance:

```text
English Meeting Sound
→ final English transcript
→ Indonesian translated text
```

Normal lifecycle:

```text
Ready → Starting → Live → Stopping → Ended
```

Outbound Meeting translation may use the last three committed own-voice translation pairs from the same live session as bounded context. Incoming remains context-free.

### Text

Standalone Indonesian ↔ English text translation with explicit direction, Translate, result review, and Copy. Document/file translation is not current scope.

### My Voice

Meeting works on day one with two built-in English voice references. My Voice is an optional high-fidelity upgrade created from the user's authorized recordings:

```text
guided recording
→ replay / accept / retry
→ GPT-SoVITS V2ProPlus training
→ held-out evaluation
→ user approval
→ approved reusable My Voice
```

### Current exclusions

General History/Saved UI, Document Translation, Audio Studio, Push to Talk, Pause/Resume, user-facing Realtime/Quality modes, Auto/Casual translation styles, additional languages, imported-audio/quick-clone My Voice modes, multiple normal voice engines, partial translated subtitles, incoming Indonesian TTS, and automatic mid-session Meeting Sound rebind are outside the current product boundary. Translation style is intentionally bounded to Natural (default) or Formal.

## Architecture

```text
Tauri 2
├─ Svelte 5 + Vite + TypeScript
├─ Rust desktop/runtime backend
└─ one canonical Python local worker
   ├─ ASR
   ├─ Indonesian ↔ English translation
   └─ GPT-SoVITS voice runtime
```

Canonical roots:

```text
EngineData/Frontend/RustApp/
EngineData/Backend/LocalWorker/WorkerRuntime/
EngineData/Backend/RuntimeAssets/
UserData/
```

Historical development material is retained by Git history rather than exposed as a second current source tree.

## Repository operating model

```text
AGENTS.md
→ execution context / work mode / semantic routing

GITHUB_RULES.md
→ Local-only authority / GitHub-first partition / atomic delivery / CI / security / retry / STOP

CONTEXT.md
→ current orientation

docs/foundation/
→ current product/system law

docs/knowledge/next-action.md
→ current continuation + exactly one next step

docs/knowledge/current-validation.md
→ proof interpretation

docs/knowledge/source-ownership.md
→ responsibility → current owner

docs/knowledge/decisions/
→ durable decisions and reasons
```

Normal GitHub work follows:

```text
PIN
→ EXECUTION CONTEXT
→ EXHAUST REMOTE_GITHUB PARTITION
→ READ MINIMUM
→ DIAGNOSE
→ TOOL + TRANSFER GATE
→ WRITE ONCE
→ VERIFY + FAILURE POLICY
→ STOP
```

## Branch model

**Local-only.**

```text
Local
→ sole active development/source/governance/CI/proof authority
```

Routine work lands directly on `Local` as one logical delivery. Do not create promotion branches, task branches, or alternate development branches as part of the normal method. A different branch lifecycle requires a new explicit user decision.

## Development entrypoints

Canonical Windows developer surface:

```text
DEV.cmd doctor
DEV.cmd setup
DEV.cmd check
DEV.cmd build
DEV.cmd test
DEV.cmd package
```

`DEV.cmd` is only a thin router. Existing npm/Cargo/uv/release owners remain authoritative; it does not create a second build system. Supported toolchain policy is recorded in `toolchain.json`.

Direct subsystem commands remain valid for targeted work. Use only proof required by the changed claim; hosted/static proof does not become native Windows acceptance.

## Release boundary

Current controlled offline distribution uses:

```text
TranslateIT-Setup.exe
TranslateIT-Payload.7z
```

Release-source validation is performed from `Local`. Setup owns validating and installing its colocated payload, private Python/runtime/model assets, and supported Meeting audio provider inputs. Normal users are not asked to run pip, manually extract runtime assets, or download core models.

Publishing a tag/GitHub Release is a separate explicit action and does not change repository branch authority.

## Security / contribution

See `SECURITY.md` and `CONTRIBUTING.md`. Public repository visibility does not itself grant reuse or redistribution rights; license policy remains an explicit owner decision.
