# TranslateIT Source Ownership

This file maps semantic responsibility to the current owner. It does not carry proof outcomes or historical implementation detail.

## Governance

| Responsibility | Owner |
|---|---|
| GitHub branch/ref/history, CI security, retry/STOP | `GITHUB_RULES.md` |
| Agent execution/routing | `AGENTS.md` |
| Proof taxonomy / development discipline | `docs/knowledge/development-discipline.md` |
| Toolchain policy | `toolchain.json` |
| Unified Windows developer routing | `DEV.cmd` → `tooling/windows-toolchain/dev.ps1` |
| Architecture vocabulary | `CONTEXT.md` |
| Product/system law | `docs/foundation/` |
| Active continuation | `docs/knowledge/next-action.md` |
| Proof interpretation | `docs/knowledge/current-validation.md` |
| Repository static governance | `tools/verify_repository.py` |
| Frontend source/bridge contracts | `EngineData/Frontend/RustApp/scripts/` validators |

## Desktop product/runtime

| Responsibility | Owner |
|---|---|
| App composition shell | `src/App.svelte` |
| Product pages | `src/pages/` |
| Product runtime composition | `src/app/bridge/runtimeProductFacade.ts` |
| Product audio / translation / setup actions | `productAudioFacade.ts`, `productTranslationFacade.ts`, `productSetupFacade.ts` |
| Meeting product mapping | `productMeetingState.ts` |
| Product readiness mapping | `productReadinessState.ts` |
| Worker capability parsing | `workerCapabilities.ts` |
| Product DTOs | `runtimeProductTypes.ts` |
| Tauri bridge calls/types | `runtimeApi.ts`, `applicationRuntimeApi.ts`, `myVoiceApi.ts`, `myVoiceBuildApi.ts` |
| Tauri window permissions | `src-tauri/capabilities/default.json` + `translation-overlay.json` |
| Frontend application state sequencing | `applicationController.ts` |
| Meeting event/reconciliation state | `meetingLiveController.ts` + `meetingReconcileReader.ts` + `reliabilityMonitor.ts` |
| Native close flow | `closeController.ts` + `nativeCloseRuntime.ts` + `closePolicy.ts` |
| Translation overlay presentation | `translationOverlay*.ts` + `pages/TranslationOverlay.svelte` |
| First-setup navigation/resume | `SetupNavigation.svelte` + `setupFlow.ts` |
| Meeting preset product state | `meetingPresetState.ts` + `MeetingPresetPanel.svelte` |
| Text cache / translation feedback | `textTranslationCache.ts`, `translationFeedbackState.ts` |
| One-shot updater frontend | `appUpdateApi.ts` + `UpdateAction.svelte` |

## Application runtime authority

Cross-feature coordination is owned by:

`EngineData/Frontend/RustApp/src-tauri/src/commands/application_runtime/`

| Responsibility | Owner |
|---|---|
| Typed public contracts | `contract.rs` |
| Cheap read-only snapshot | `snapshot.rs` |
| Typed subsystem projection | `summaries.rs` |
| Global lifecycle/capabilities/resources/problems | `lifecycle.rs`, `capabilities.rs`, `resources.rs`, `problems.rs` |
| Parameterless product intents | `intents.rs` |
| Typed parameterized mutations | `mutations.rs` |
| Runtime events | `events.rs` |
| Safe app shutdown coordination | `shutdown.rs` |

Canonical shared-resource owners:

- `translateit_application_meeting`
- `translateit_mic_test`
- `translateit_voice_recording`

Domain algorithms remain in their domain modules; ApplicationRuntime coordinates but does not absorb them.

## Meeting / audio

| Responsibility | Owner |
|---|---|
| Meeting public status facade | `commands/meeting_session.rs` |
| Start/Stop lifecycle transactions | `commands/meeting_session/lifecycle.rs` + `engine/runtime_state.rs` |
| Meeting preflight/status | `meeting_session/preflight.rs` + `session_state.rs` |
| Outbound/incoming pipelines | `meeting_session/outbound_pipeline.rs`, `incoming_pipeline.rs`, `incoming_deferred.rs` |
| Meeting consumer ownership | `meeting_session/consumer_runtime.rs` |
| Committed turns / last-session review | `meeting_session/committed_turns.rs` |
| Physical capture / VAD | `engine/audio/` |
| Optional Meeting Sound capture | `engine/audio/meeting_sound_capture.rs` |
| Meeting output delivery | `engine/audio/meeting_output*.rs` |
| Virtual microphone route | `commands/virtual_mic_route.rs` |
| Device/settings persistence | `commands/settings.rs` + `engine/settings.rs` |

## Local AI / voice

| Responsibility | Owner |
|---|---|
| Canonical worker entry | `EngineData/Backend/LocalWorker/WorkerRuntime/realtime_local_worker.py` |
| Worker orchestration/I/O | `realtime_local_worker_base.py`, `worker_io_runtime.py`, `worker_runtime_common.py` |
| Helper process/transport/readiness | Rust `commands/helper_bridge*` |
| Canonical MiLMMT translation | `milmmt_translation_provider.py` |
| Model inventory/staging | `model_manifest.json` + `prepare_model_assets*.py` |
| My Voice recording/review | Rust `commands/voice_lab_recording.rs` |
| My Voice build/evaluation | Rust `commands/voice_lab_build.rs` + `voice_lab_build.py` + `voice_lab_gpt_sovits_build.py` |
| My Voice live inference | `voice_lab_gpt_sovits.py` + `worker_io_runtime.py` |
| Voice storage/package promotion | Rust `commands/voice_lab/storage.rs` |

## Release / packaging

| Responsibility | Owner |
|---|---|
| Release orchestration | `scripts/build_release.ps1` |
| Release input staging | `stage_release_inputs*.ps1` |
| Payload construction | `build_r3_external_payload.py` |
| Package/source validation | `validate_release_*.mjs`, `validate_tauri_package_preflight.mjs` |
| Installer behavior | `src-tauri/windows/` + Tauri release config |
| Updater signing/artifacts | `build_release.ps1` + `tauri.release.conf.json` |
| Third-party notices | `generate_third_party_notices.mjs` |

## Navigation questions

```text
Who owns this?        → source-ownership.md
What must it do?      → docs/foundation/
What is active?       → next-action.md
What is proven?       → current-validation.md
Why was it chosen?    → decisions/
What happens now?     → current source + matching proof
```

Historical paths never become current owners merely because an old report references them.


### Bridge proof ownership

Command-name/response-shape parity is owned by `validate_bridge_contract.mjs`. Rust-command argument-key parity is owned by `validate_tauri_command_args.mjs`. Frontend bridge wrapper reachability is owned by `validate_runtime_api_usage.mjs`. These checks are complementary: a registered command can still be waste if its frontend wrapper has no consumer, and a command name can still match while its invocation arguments drift.
