# TranslateIT Source Ownership

This file maps semantic responsibility to the current owner. It does not carry milestone status or proof results.

## Governance

| Responsibility | Owner |
|---|---|
| GitHub branch/ref/history, atomic delivery, transfer, CI security, retry, STOP | `GITHUB_RULES.md` |
| Agent execution context, work mode, routing, skill budget | `AGENTS.md` |
| Development discipline / proof taxonomy / ownership economy | `docs/knowledge/development-discipline.md` |
| Supported developer toolchain policy | `toolchain.json` |
| Unified Windows developer routing | `DEV.cmd` → `tooling/windows-toolchain/dev.ps1` |
| Stable orientation / current architecture vocabulary | `CONTEXT.md` |
| Product/system law | `docs/foundation/` |
| Active continuation | `docs/knowledge/next-action.md` |
| Current proof interpretation | `docs/knowledge/current-validation.md` |
| Durable decisions/reasons | `docs/knowledge/decisions/` |
| Repository static governance enforcement | `tools/verify_repository.py` |
| Frontend bridge/source-health contracts | `EngineData/Frontend/RustApp/scripts/validate_bridge_contract.mjs`, `validate_frontend_reachability.mjs`, `validate_source_size_budget.mjs` |

## Desktop product/runtime

| Responsibility | Current owner |
|---|---|
| App root / workspace composition | `EngineData/Frontend/RustApp/src/App.svelte` |
| Product pages | `EngineData/Frontend/RustApp/src/pages/` |
| Product runtime orchestration/actions | `EngineData/Frontend/RustApp/src/app/bridge/runtimeProductFacade.ts` |
| Product readiness / Meeting-state mapping | `EngineData/Frontend/RustApp/src/app/bridge/runtimeProductState.ts` |
| Product runtime DTOs | `EngineData/Frontend/RustApp/src/app/bridge/runtimeProductTypes.ts` |
| Tauri command bridge calls/types | `EngineData/Frontend/RustApp/src/app/bridge/runtimeApi.ts`, `myVoiceApi.ts`, `myVoiceBuildApi.ts` |
| Meeting frontend polling / committed-turn refresh | `EngineData/Frontend/RustApp/src/app/runtime/meetingPoll.ts` |
| Native safe-close I/O / close decision policy | `EngineData/Frontend/RustApp/src/app/runtime/nativeCloseRuntime.ts` + `closePolicy.ts` |
| Rust app bootstrap / command registration | `EngineData/Frontend/RustApp/src-tauri/src/app_bootstrap.rs`, `commands/registry.rs` |
| Meeting application lifecycle | `EngineData/Frontend/RustApp/src-tauri/src/commands/meeting_session.rs` + `engine/runtime_state.rs` |
| Settings persistence/runtime settings | Rust `commands/settings.rs` + `engine/settings.rs` and frontend settings surface |

## Local AI / translation / voice

| Responsibility | Current owner |
|---|---|
| Canonical worker entry | `EngineData/Backend/LocalWorker/WorkerRuntime/realtime_local_worker.py` |
| Worker orchestration/base | `realtime_local_worker_base.py`, `worker_io_runtime.py`, `worker_runtime_common.py` |
| Canonical MiLMMT translation | `milmmt_translation_provider.py` + translation contract validator |
| Model inventory/staging contract | `model_manifest.json`, `prepare_model_assets*.py` |
| My Voice build/inference | `voice_lab_build.py`, `voice_lab_gpt_sovits.py`, Rust `commands/voice_lab*.rs` |
| Built-in Meeting voice selection/reference assets | Rust `commands/voice_lab_build.rs`, `RuntimeAssets/Voice/BuiltInVoices/`, frontend My Voice bridge/page |

## Windows audio

| Responsibility | Current owner |
|---|---|
| Physical capture / finalized utterance / VAD | Rust `engine/audio/` |
| Optional Meeting Sound capture | `engine/audio/meeting_sound_capture.rs` |
| Meeting output delivery | `engine/audio/meeting_output.rs` |
| Virtual route detection/setup behavior | `commands/virtual_mic_route.rs` + audio runtime owners |
| Provider package delivery | release/package owner, not audio runtime |

## Release / packaging

| Responsibility | Current owner |
|---|---|
| Release orchestration | `EngineData/Frontend/RustApp/scripts/build_release.ps1` |
| Release input staging | `stage_release_inputs.ps1` + `stage_release_inputs_impl.ps1` |
| Payload construction | `build_r3_external_payload.py` |
| Package/source contract validation | `validate_release_*.mjs`, `validate_tauri_package_preflight.mjs` |
| Third-party notices | `generate_third_party_notices.mjs` + staged license material |
| Installer payload behavior | `src-tauri/windows/` + Tauri release config |

## Navigation questions

```text
Who owns this?        → source-ownership.md
What must it do?      → docs/foundation/
What is active?       → next-action.md
What is proven?       → current-validation.md
Why was it chosen?    → decisions/
What happens now?     → current source + matching proof
```

Historical paths/branch names never become current owners merely because an old report references them.
