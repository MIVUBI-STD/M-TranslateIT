# Next Action

## Current Status

`Local` is the sole active source, development, governance, CI, proof, and continuation authority.

Remote source work now includes the bounded productivity set: command-palette Quick Translate Clipboard, Smart Copy, searchable/pinnable live Meeting turns, temporary last-session review, user-driven Meeting presets with detected-app suggestions, searchable/CSV terminology maintenance, bounded in-memory Text cache, and opt-in local translation feedback with explicit review/export/clear.

The final remote zero-waste pass removed unused productivity state, closed the write-only feedback loop, removed Command Palette accessibility warnings, sanitizes malformed local preset/feedback storage, and moves Meeting preset application into one Rust command that checks runtime ownership, validates both audio devices, sanitizes the candidate, and persists once.

Reliability remains crash-safe and bounded: watchdog, device-loss handling, redacted support export, incident history, long-session pressure monitoring, centralized live-monitor ownership, one canonical local AI pipeline, and one-shot signed updater.

Outbound Meeting orchestration now separates local AI preparation from Meeting playback through one generation-bound playback runtime with a single pending slot. Capture/ASR/translation/TTS remain canonical and serialized, while the next prepared turn may progress during prior playback without creating an unbounded audio backlog. Actual latency improvement remains TARGET_WINDOWS evidence.

## Active Boundary

Preserve:

- one canonical local translation pipeline;
- bounded queues, transcript state, diagnostics, incidents, cache, feedback, and temp data;
- fail-closed session authority and at-most-once Meeting output;
- explicit device selection with no silent hot-swap or preset auto-apply;
- privacy-redacted diagnostics;
- no cloud fallback, second normal translation/TTS engine, background updater, clipboard watcher, persistent translation cache, Auto Language Detection, or Screen Translation;
- Quick Translate without a default global OS shortcut.

Do not expand productivity breadth without a concrete source defect or new explicit product decision.

## Next Step

Remote feature/source expansion and the application-runtime modularization pass are source-complete. The current source now includes typed ApplicationRuntime authority, distinct shared-resource owners, event-first Meeting reconciliation, split frontend controllers, split product facades/state policies, closed superseded mutation entrypoints, and architecture regression guards.

Before TARGET_WINDOWS acceptance, obtain matching hosted source proof for the exact current Local SHA when CI is available. If CI exposes a concrete regression, repair only that regression; do not reopen broad architecture expansion.

Then proceed to `TARGET_WINDOWS` acceptance using `docs/knowledge/operations/target-windows-performance.md`. Validate the installed app on the intended Windows machine: physical microphone, TranslateIT Meeting Microphone route, Zoom/Teams/Meet/Discord reception, real GPU/CPU/RAM/VRAM behavior, Start→Live and speech→playback latency, long-session stability, mixed-DPI overlay, sleep/wake, unplug/replug, My Voice audible quality, updater install, and clean-machine setup.

Source/hosted CI proof must not be reported as native-device acceptance.
