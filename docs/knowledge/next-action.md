# Next Action

## Current Status

`Local` is the sole active source, development, governance, CI, proof, and continuation authority.

Current source includes Spoken Terms, Meeting app detection, recent/paused captions, direction switching, Natural/Formal style, Audio Quality Guard, Noise Suppression, command-palette Quick Translate Clipboard, transcript export, temporary last-session review, user-driven Meeting presets with detected-app suggestions, searchable/CSV terminology maintenance, bounded in-memory Text translation cache, local opt-in translation issue feedback, caption readability controls, and bounded My Voice pace preservation.

Reliability includes crash-safe startup recovery, bounded watchdog, fail-closed device loss, redacted support export, bounded incident history, long-session pressure monitoring, and centralized live-monitor ownership.

Architecture remains one Tauri 2 / Svelte 5 / Rust desktop app plus one canonical Python worker for Faster-Whisper ASR, MiLMMT ID↔EN translation, and GPT-SoVITS voice work. Updates remain a signed one-shot startup check with no updater daemon.

## Active Boundary

Preserve:

- one canonical local translation pipeline;
- bounded queues, transcript state, diagnostics, incidents, cache, feedback, and temp data;
- fail-closed session authority and at-most-once Meeting output;
- explicit device selection with no silent hot-swap or preset auto-apply;
- privacy-redacted diagnostics;
- no cloud fallback, second normal translation/TTS engine, background updater, clipboard watcher, persistent translation cache, Auto Language Detection, or Screen Translation;
- Quick Translate without a default global OS shortcut.

Remote proof never implies native Windows, GPU, audio-route, installer, meeting-app, mixed-DPI, sleep/wake, device-loss, audible My Voice, updater-install, or clean-machine acceptance.

## Next Step

Keep `Local` green. Resolve exact current-HEAD CI/governance regressions first. Then audit only falsifiable high-value defects: dead/unreachable reliability code, duplicate polling ownership, filesystem I/O under runtime locks, unbounded growth, privacy leakage, multi-instance recovery conflicts, bridge/API drift, source-size regressions, and incorrect long-session semantics.

Do not expand productivity breadth further unless a concrete gap is demonstrated. If no material remote defect remains, preserve the repo for TARGET_WINDOWS acceptance using `docs/knowledge/operations/target-windows-performance.md`.
