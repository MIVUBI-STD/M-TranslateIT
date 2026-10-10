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

## Start here and file ownership

For ChatGPT source work use GitHub branch `Local`: `AGENTS.md` → `GITHUB_RULES.md` → [Documentation](docs/README.md) → canonical owner → source review → STOP. **No cloud compute, cloud AI, hosted CI or hosted builds.** Local Windows build and acceptance are one later integrated phase, not incremental PC tasks.

| Root path | Single responsibility | Detail |
|---|---|---|
| `README.md` | Human entry/navigation | Domain routers, not duplicated owner rules |
| `AGENTS.md` | Mode, context, specialist budget | [.agents/](.agents/README.md) |
| `GITHUB_RULES.md` | GitHub ref, mutation, evidence and retry | Existing root policy |
| `CONTEXT.md` | Stable product/architecture orientation | No active work state |
| `CONTRIBUTING.md`, `SECURITY.md` | Contribution and trust boundaries | Existing root contracts |
| `toolchain.json`, `DEV.cmd` | Toolchain policy and optional thin launcher | [Tooling](tooling/README.md) |
| `docs/` | Durable product/system/developer knowledge | [Documentation](docs/README.md) |
| `planning/` | Current development intent/next action | [Development](planning/README.md) |
| `EngineData/` | Tauri frontend, Rust runtime, Python AI worker/assets | [Source owners](docs/knowledge/source-ownership.md) |
| `UserData/` | Private runtime/user storage placeholders only | [Storage](UserData/README.md) |
| `tools/` | Repository/quality validation and source-derived planning | [Tools](tools/README.md) |
| `.agents/`, `.github/` | Skill/permission registry and GitHub configuration | [Agents](.agents/README.md) |

Naming and update/move rules: [Canonical Naming](docs/system/canonical-naming.md). Source is one owner per behavior; Git history carries revisions. Do not fabricate parallel `workspace/` or `experiments/` folders for this Windows desktop product.

## Repository operating model

Read [documentation routing](docs/README.md) first for source/owner questions. Stable document identities and explicit Markdown links power a derived, bounded catalog; `python tools/repository_knowledge.py --summary` shows its current surface. Existing runtime validators remain authoritative for their respective wire contracts.

```text
AGENTS.md
→ execution context / work mode / semantic routing

GITHUB_RULES.md
→ Local-only authority / GitHub-first partition / atomic delivery / CI / security / retry / STOP

CONTEXT.md
→ current orientation

docs/foundation/
→ current product/system law

planning/development.md
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

## GitHub Actions policy

Eight GitHub Actions workflow definitions are retained as **manual-only, inactive templates**. Under the no-cloud decision, do not dispatch them or depend on hosted execution. `Local` owns development; default `main` is final-only and unchanged. Source review can close source-only work; compiler, package, AI model and device checks remain `NOT EXECUTED` until the planned integrated local build/test phase. Publication and promotion remain separate approvals.

## Branch model

**Local-only development; `main` reserved for final results.**

```text
Local  = current development, source, governance, validation and continuation
main   = GitHub default branch; accepted final/release result only
```

Routine work lands directly on `Local` as one logical delivery. Do not create task/promotion branches or promote work to `main` until acceptance and explicit user authorization. The GitHub default branch does not determine which branch owns in-progress development.

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

`DEV.cmd` is an optional developer convenience, not a step ChatGPT assigns to the user's PC during development. Existing npm/Cargo/uv/release owners remain authoritative; it does not create a second build system. Supported toolchain policy is recorded in `toolchain.json`.

Direct subsystem commands belong to an explicitly available local execution environment; GitHub source review alone never proves a build or Windows acceptance.

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
