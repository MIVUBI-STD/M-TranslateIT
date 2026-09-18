# Tauri Command Modules

This folder contains the Tauri command boundary for the current Meeting / Text / My Voice / Settings product.

## Active modules

- `meeting_session.rs` — public Meeting status/Start/Stop orchestration and lifecycle coordination; child modules own committed turns, consumer thread lifecycle, lane pipelines, deferred incoming work, and internal session status/preflight stores.
- `helper_bridge.rs` / `helper_bridge_runtime.rs` — persistent local Python worker bridge and scheduler; `helper_bridge/functional_readiness.rs` owns the required outbound functional-readiness cache and response-validation policy.
- `audio.rs` — microphone/device status and device checks used by setup.
- `mic_test.rs` — Microphone Test Start/Stop wrappers.
- `runtime.rs` / `runtime_inventory.rs` — runtime readiness and explicit model-presence verification.
- `settings.rs` — runtime settings load/save and audio-device selection.
- `text_translation.rs` — standalone Text translation through the same worker.
- `virtual_mic_route.rs` — managed virtual microphone used by the meeting application.
- `voice_lab.rs`, `voice_lab_recording.rs`, `voice_lab_build.rs` — legacy internal protocol/storage implementation identifiers for the product feature now named **My Voice**. These names remain only where changing them requires an explicit compatibility migration; they are not product vocabulary.
- `diagnostic_trace.rs` — bounded diagnostics trace command support.
- `bridge_paths.rs` — packaged private Python/runtime path resolution.
- `registry.rs` — Tauri invoke registration list.

## Naming rule

Prefer names that describe the responsibility directly. New product-facing or semantic source must use **My Voice** (`MyVoice`, `myVoice`, or `my_voice` according to language convention). Do not introduce new `VoiceLab` product terminology.

Existing `voice_lab` command names, state/error identifiers, and on-disk storage paths may remain only where changing them would require an explicit compatibility migration. Product-facing bridges must translate any legacy display copy to **My Voice** without rewriting those machine-facing identifiers.

A source rename is complete only after the retained frontend, Python, Rust, repository, and MiLMMT checks pass on the candidate head. New command wrappers should be added only when a current product requirement and direct caller require them.
