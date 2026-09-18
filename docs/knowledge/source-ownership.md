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
| Worker capability response parsing / diagnostic display mapping | `EngineData/Frontend/RustApp/src/app/bridge/workerCapabilities.ts` |
| Product runtime DTOs | `EngineData/Frontend/RustApp/src/app/bridge/runtimeProductTypes.ts` |
| Meeting performance Diagnostics | `EngineData/Frontend/RustApp/src/components/settings/MeetingPerformanceDiagnostics.svelte` + existing Meeting status DTOs |
| Tauri command bridge calls/types | `EngineData/Frontend/RustApp/src/app/bridge/runtimeApi.ts`, `myVoiceApi.ts`, `myVoiceBuildApi.ts` |
| Meeting frontend polling / committed-turn refresh | `EngineData/Frontend/RustApp/src/app/runtime/meetingPoll.ts` |
| Native close dialog presentation | `EngineData/Frontend/RustApp/src/components/runtime/NativeCloseDialog.svelte` |
| First-setup navigation presentation | `EngineData/Frontend/RustApp/src/components/setup/SetupNavigation.svelte` |
| Native safe-close I/O / close decision policy | `EngineData/Frontend/RustApp/src/app/runtime/nativeCloseRuntime.ts` + `closePolicy.ts` |
| Rust app bootstrap / command registration | `EngineData/Frontend/RustApp/src-tauri/src/app_bootstrap.rs`, `commands/registry.rs` |
| Meeting public command/status facade | `EngineData/Frontend/RustApp/src-tauri/src/commands/meeting_session.rs` |
| Meeting Start/Stop lifecycle transactions | `commands/meeting_session/lifecycle.rs` + `engine/runtime_state.rs` |
| Meeting preflight/status derivation | `commands/meeting_session/preflight.rs` + `session_state.rs` |
| Meeting consumer ownership | `commands/meeting_session/consumer_runtime.rs` |
| Meeting outbound/incoming processing | `commands/meeting_session/outbound_pipeline.rs`, `incoming_pipeline.rs`, `incoming_deferred.rs` |
| Meeting self-output suppression / optional incoming activation | `commands/meeting_session/suppression.rs`, `incoming_activation.rs` |
| Settings persistence/runtime settings | Rust `commands/settings.rs` + `engine/settings.rs` and frontend settings surface |

## Local AI / translation / voice

| Responsibility | Current owner |
|---|---|
| Canonical worker entry | `EngineData/Backend/LocalWorker/WorkerRuntime/realtime_local_worker.py` |
| Worker orchestration/base | `realtime_local_worker_base.py`, `worker_io_runtime.py`, `worker_runtime_common.py` |
| Rust helper public bridge/process lifecycle | `commands/helper_bridge.rs` |
| Helper worker transactions + bounded recovery | `commands/helper_bridge/transport.rs` |
| Helper request classification / Meeting priority policy | `commands/helper_bridge/request_policy.rs` |
| Required outbound functional readiness / actor binding | `commands/helper_bridge/functional_readiness.rs` |
| Required outbound end-to-end AI probe | `commands/helper_bridge/functional_probe.rs` |
| Helper runtime state / worker I/O / deadlines | `commands/helper_bridge_runtime.rs` |
| Helper scheduler / admission / permit priority | `commands/helper_bridge_runtime/scheduler.rs` |
| Canonical MiLMMT translation | `milmmt_translation_provider.py` + translation contract validator |
| Model inventory/staging contract | `model_manifest.json`, `prepare_model_assets*.py` |
| My Voice build/inference | `voice_lab_build.py`, `voice_lab_gpt_sovits.py`, Rust `commands/voice_lab*.rs` |
| My Voice dataset/storage/package validation/promotion | Rust `commands/voice_lab/storage.rs` |
| My Voice guided recording/review transaction | Rust `commands/voice_lab_recording.rs` |
| My Voice held-out evaluation contract | Rust `commands/voice_lab_build/evaluation.rs` |
| Built-in Meeting voice selection/reference assets | Rust `commands/voice_lab_build.rs`, `RuntimeAssets/Voice/BuiltInVoices/`, frontend My Voice bridge/page |

## Windows audio

| Responsibility | Current owner |
|---|---|
| Physical capture / finalized utterance / VAD | Rust `engine/audio/` |
| Optional Meeting Sound capture | `engine/audio/meeting_sound_capture.rs` |
| Meeting output delivery / CPAL stream lifecycle | `engine/audio/meeting_output.rs` + `meeting_output_runtime.rs` |
| Meeting output WAV decode / resample / delivery deadline math | `engine/audio/meeting_output_runtime/audio_format.rs` |
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
