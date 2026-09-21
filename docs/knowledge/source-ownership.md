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

| Responsibility | Owner |
|---|---|
| App root / workspace composition | `EngineData/Frontend/RustApp/src/App.svelte` |
| Product pages | `EngineData/Frontend/RustApp/src/pages/` |
| Product runtime composition | `EngineData/Frontend/RustApp/src/app/bridge/runtimeProductFacade.ts` |
| Product audio actions | `EngineData/Frontend/RustApp/src/app/bridge/productAudioFacade.ts` |
| Product translation actions | `EngineData/Frontend/RustApp/src/app/bridge/productTranslationFacade.ts` |
| Product setup / recovery actions | `EngineData/Frontend/RustApp/src/app/bridge/productSetupFacade.ts` |
| Meeting product-state mapping | `EngineData/Frontend/RustApp/src/app/bridge/productMeetingState.ts` |
| Product readiness mapping | `EngineData/Frontend/RustApp/src/app/bridge/productReadinessState.ts` |
| Worker capability parsing / diagnostic display | `EngineData/Frontend/RustApp/src/app/bridge/workerCapabilities.ts` |
| Product runtime DTOs | `EngineData/Frontend/RustApp/src/app/bridge/runtimeProductTypes.ts` |
| Meeting performance diagnostics | `EngineData/Frontend/RustApp/src/components/settings/MeetingPerformanceDiagnostics.svelte` + existing Meeting status DTOs |
| Tauri command bridge calls/types | `EngineData/Frontend/RustApp/src/app/bridge/runtimeApi.ts`, `myVoiceApi.ts`, `myVoiceBuildApi.ts` |
| Meeting frontend event/reconciliation coordination | `EngineData/Frontend/RustApp/src/app/runtime/meetingLiveController.ts` + `meetingReconcileReader.ts` + `reliabilityMonitor.ts` |
| Command palette + Quick Translate action surface | `src/components/runtime/ProductivityActions.svelte`, `CommandPalette.svelte`, `src/app/runtime/productCommandRegistry.ts` |
| Bounded Text translation cache | `src/app/runtime/textTranslationCache.ts` |
| Meeting preset state / provider suggestions | `src/app/runtime/meetingPresetState.ts` + `src/components/meeting/MeetingPresetPanel.svelte` |
| Atomic Meeting preset validation/persistence | Rust `commands/settings.rs::apply_meeting_preset` |
| Temporary last-session review | `src/components/meeting/MeetingSessionReview.svelte` + Rust `commands/meeting_session/committed_turns.rs` |
| Translation feedback state/review | `src/app/runtime/translationFeedbackState.ts`, `src/components/text/TranslationFeedback.svelte`, `src/components/settings/TranslationFeedbackReview.svelte` |
| Native close dialog presentation | `EngineData/Frontend/RustApp/src/components/runtime/NativeCloseDialog.svelte` |
| One-shot app update frontend lifecycle | `EngineData/Frontend/RustApp/src/app/update/appUpdateApi.ts` + `src/components/runtime/UpdateAction.svelte` |
| Signed native update check/install policy | `EngineData/Frontend/RustApp/src-tauri/src/commands/app_update.rs` |
| First-setup navigation | `EngineData/Frontend/RustApp/src/components/setup/SetupNavigation.svelte` |
| Setup checkpoint decode/encode + safe resume policy | `EngineData/Frontend/RustApp/src/app/runtime/setupFlow.ts` |
| Native safe-close / close policy | `EngineData/Frontend/RustApp/src/app/runtime/closeController.ts` + `nativeCloseRuntime.ts` + `closePolicy.ts` |
| Rust app bootstrap / command registration | `EngineData/Frontend/RustApp/src-tauri/src/app_bootstrap.rs`, `commands/registry.rs` |
| Native process-exit fail-safe / helper shutdown | `commands/application_runtime/shutdown.rs`; `src-tauri/src/main.rs` delegates only |
| Meeting public command/status facade | `EngineData/Frontend/RustApp/src-tauri/src/commands/meeting_session.rs` |
| Fresh Meeting virtual-route preparation | `EngineData/Frontend/RustApp/src-tauri/src/commands/runtime.rs` → `commands/virtual_mic_route.rs` |
| Generation-bound route binding | `commands/meeting_session/lifecycle.rs` → `commands/virtual_mic_route.rs` |
| Meeting Start/Stop lifecycle transactions | `commands/meeting_session/lifecycle.rs` + `engine/runtime_state.rs` |
| Meeting preflight/status derivation | `commands/meeting_session/preflight.rs` + `session_state.rs` |
| Meeting consumer ownership | `commands/meeting_session/consumer_runtime.rs` |
| Meeting outbound/incoming processing | `commands/meeting_session/outbound_pipeline.rs`, `incoming_pipeline.rs`, `incoming_deferred.rs` |
| Meeting self-output suppression / optional incoming activation | `commands/meeting_session/suppression.rs`, `incoming_activation.rs` |
| Settings persistence/runtime settings | Rust `commands/settings.rs` + `engine/settings.rs` and frontend settings surface |

## Local AI / translation / voice

| Responsibility | Owner |
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
| My Voice one-shot build child | `voice_lab_build.py` + Rust `commands/voice_lab_build.rs` |
| My Voice GPT-SoVITS build/training/evaluation pipeline | `voice_lab_gpt_sovits_build.py` |
| My Voice live actor validation/runtime/synthesis | `voice_lab_gpt_sovits.py` + `worker_io_runtime.py` |
| My Voice dataset/storage/package validation/promotion | Rust `commands/voice_lab/storage.rs` |
| My Voice guided recording/review transaction | Rust `commands/voice_lab_recording.rs` |
| My Voice guided prompt corpus | Rust `commands/voice_lab_recording/guided_lines.rs` |
| My Voice accepted-recording domain query | Rust `commands/voice_lab_recording.rs` consumed by `voice_lab_build.rs` |
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
| Updater release signing / artifact contract | `scripts/build_release.ps1` + `src-tauri/tauri.release.conf.json` + `scripts/tests/updater_contract.test.ts` |

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


## Application runtime kernel

Cross-feature coordination is owned by:

```text
EngineData/Frontend/RustApp/src-tauri/src/commands/application_runtime/
```

Ownership boundaries:

- `contract.rs`: public application-runtime contracts only.
- `summaries.rs`: typed adapters from domain status into stable subsystem summaries.
- `lifecycle.rs`: derives global application lifecycle from summaries only.
- `capabilities.rs`: resolves product-level capability availability from summaries/resources.
- `resources.rs`: derives cross-feature resource availability; it must not duplicate the lower-level runtime session authority store.
- `problems.rs`: normalizes domain blockers into product-level problems.
- `snapshot.rs`: read-only aggregation. It must not execute product actions.
- `intents.rs`: the application-runtime owner for parameterless cross-feature product-intent routing.
- `mutations.rs`: typed parameterized cross-feature mutations (audio selection, voice recording/build lifecycle) and post-mutation snapshot publication.
- `events.rs`: application-runtime event publication only.
- `mod.rs`: thin Tauri/public surface.

Domain implementations remain owned by their existing modules (Meeting, audio, helper worker, voice, translation, overlay, settings). The application runtime coordinates them through typed summaries and public commands; it must not absorb domain algorithms or UI behavior.

The architecture gate `npm run validate:application-runtime` enforces the modular boundary and rejects JSON-typed orchestration, oversized kernel modules, UI/window coupling, and action execution from the read-only snapshot owner.


### Shared audio owner identities

The runtime session authority distinguishes these owners:

- `translateit_application_meeting` — Meeting translation.
- `translateit_mic_test` — Mic Test capture.
- `translateit_voice_recording` — My Voice guided recording.

Do not collapse Mic Test and My Voice back into a generic live-capture owner. They may share the same lower-level audio engine, but product lifecycle, stop authority, Settings lock copy, and recovery routing require distinct ownership identities.


### Shutdown ownership

Application exit coordination is owned by `application_runtime/shutdown.rs`. `main.rs` only delegates the exit decision and restores the main window when shutdown is blocked. Close UI policy consumes the canonical ApplicationSnapshot plus My Voice pending-review state; it must not independently reconstruct Meeting/helper/Voice Build ownership through separate probes.


### Meeting runtime events

Backend Meeting change-token publication is owned by `engine/runtime_events.rs`. Authoritative transcript mutation sites emit lightweight events; they do not serialize transcript text into events. `reliabilityMonitor.ts` owns frontend subscription, debounce, and the slow reconciliation timer. `meetingReconcileReader.ts` is the authoritative Meeting fetch/reconciliation reader; event-first freshness is coordinated by `meetingLiveController.ts` / `reliabilityMonitor.ts`.


### Frontend product-state ownership

`src/app/runtime/applicationController.ts` owns the frontend product-runtime snapshot and refresh sequencing. `runtimeProductFacade.ts` remains a product mapping/query layer; it does not own reactive application state. `App.svelte` is the composition shell and must not duplicate subsystem runtime state.


### Frontend shell controllers

- `applicationController.ts` owns product snapshot/state sequencing.
- `meetingLiveController.ts` owns Meeting live transcript/overlay reconciliation state.
- `closeController.ts` owns close-dialog and native-close coordination state.
- `App.svelte` owns composition and transient shell interaction only.

### Product facade modules

- `productAudioFacade.ts` owns product-level audio selection/probe mapping.
- `productTranslationFacade.ts` owns standalone product translation mapping.
- `productSetupFacade.ts` owns setup/readiness/recovery product actions.
- `runtimeProductFacade.ts` composes/re-exports those modules and owns only the cross-domain product snapshot plus Meeting product action mapping.


### Product-state mapping ownership

`productMeetingState.ts` owns Meeting-facing product state mapping. `productReadinessState.ts` owns readiness/capability presentation policy. `workerCapabilities.ts` owns decoding worker status evidence. `runtimeProductState.ts` is not an implementation owner; it exists only to preserve stable imports while modules remain split.
