# Next Action

## Current Status

`Local` is the sole active source, development, governance, CI, proof, and continuation authority.

Source-side feature work already includes Spoken Terms, Meeting auto detection, recent caption history, Pause Captions, quick direction switching, Natural/Formal translation style, Audio Quality Guard, Noise Suppression, user-facing Quick Translate Clipboard through the command palette with no default global OS shortcut, explicit transcript export, temporary last-session review, user-driven Meeting presets with detected-app suggestions, searchable/CSV terminology maintenance, a bounded in-memory Text translation cache, local opt-in translation issue feedback, bounded caption readability controls, and bounded My Voice speaking-pace preservation.

Reliability hardening is also implemented: crash-safe startup recovery, bounded watchdog, fail-closed device-loss handling, redacted support export, bounded incident history, long-session pressure monitoring, and centralized live-monitor ownership.

Architecture remains one Tauri 2 / Svelte 5 / Rust desktop application plus one canonical Python worker for Faster-Whisper ASR, MiLMMT ID↔EN translation, and GPT-SoVITS voice work. The updater is a signed one-shot startup check; no updater daemon or permanent polling service is allowed.

GitHub Actions are authoritative for exact current SHA/run proof. Source/CI proof never implies native Windows, GPU, audio-route, installer, or clean-machine acceptance.

## Active Boundary

Continue remotely only for concrete source-level defects. Preserve:

- one canonical local translation pipeline;
- bounded queues, transcript state, diagnostics, incidents, counters, and temp data;
- fail-closed generation/session authority and at-most-once Meeting output;
- explicit device selection with no silent hot-swap;
- privacy-redacted diagnostics with no transcript/audio/voice-reference leakage;
- no cloud fallback, second normal translation/TTS engine, background updater, or duplicate capture pipeline;
- Quick Translate without a default global OS shortcut;
- no Auto Language Detection or Screen Translation;
- no speculative latency/DSP work before TARGET_WINDOWS measurement.

The user is intentionally postponing local/native testing. Do not claim real microphone, virtual-mic routing, Zoom/Teams/Meet reception, GPU throughput, mixed-DPI captions, sleep/wake, unplug/replug, audible My Voice quality, updater install flow, or clean-machine behavior from remote source proof.

## Next Step

Keep `Local` synchronized and green. First resolve any exact current-HEAD CI/governance regression. Then audit only falsifiable high-value defects: dead/unreachable reliability code, duplicate polling/monitor ownership, filesystem I/O under runtime locks, unbounded state/log/temp growth, privacy leakage, multi-instance recovery conflicts, bridge/API drift, source-size regressions, and incorrect long-session semantics.

Productivity expansion is now bounded: do not add another history database, background clipboard watcher, preset auto-apply, persistent translation cache, or parallel translation workflow. Implement only defects that can be demonstrated from source or CI. If no material remote defect remains, stop feature expansion and preserve the repo for TARGET_WINDOWS acceptance using `docs/knowledge/operations/target-windows-performance.md`.
