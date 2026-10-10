# TranslateIT Development

## Current Status

`Local` owns active development, source, governance, verification planning, continuation, and release-source validation. The GitHub default branch `main` remains reserved for a finalized, separately approved result; it is not a target for unfinished changes.

Remote source work is source-complete for the current scope: bounded productivity features, crash-safe reliability/watchdog paths, one canonical local AI pipeline, one-shot signed updater, generation-bound Meeting playback, typed ApplicationRuntime coordination, distinct Meeting/Mic Test/My Voice resource owners, event-first Meeting reconciliation, modular frontend controllers, split product facades/state policies, and closed superseded mutation entrypoints.

Actual latency, device routing, audio quality, GPU behavior, and installed-app behavior remain TARGET_WINDOWS evidence.

## Active Boundary

Preserve:

- one canonical local translation pipeline;
- bounded queues/transcripts/diagnostics/cache/feedback/temp state;
- fail-closed session authority and at-most-once Meeting output;
- explicit device selection with no silent hot-swap;
- privacy-redacted diagnostics;
- no cloud fallback, second normal translation/TTS engine, background updater, clipboard watcher, persistent translation cache, Auto Language Detection, or Screen Translation;
- Quick Translate without a default global OS shortcut.

Do not add features or broad architecture work without a concrete defect or explicit product decision.

## Next Step

Batch 5 source work adds `tools/repository_benchmark.py` and regression cases over existing agent/permission/context/impact owners. No actual model-routing accuracy, speedup or native PASS is implied; executable proof remains unverified.

Next: Quick Voice Preview Python child plus Rust resource-owned generator/cancellation and typed frontend bridge are source-wired. My Voice page controls and UI playback remain NOT IMPLEMENTED. Preview must never be selectable in Meeting. Audit model and regression contracts before grouped local acceptance; no cloud execution or incremental PC tests. `main` stays final-only.

Eight GitHub Actions workflow definitions remain **inactive** under the no-cloud policy. Their Code Health, repository/quality/lock and Windows release gates describe future local acceptance requirements, not current executable PASS. If source review finds a concrete regression, repair only its canonical owner.

After static source/integration review and locked package-input preparation, assess the **integrated test-ready candidate** gate in `docs/knowledge/operations/target-windows-performance.md`. No per-change tests on the user's PC: a single grouped local build/package/install + `TARGET_WINDOWS` pass will establish executable proof. Until actually run, build/native results remain unknown.

Source/hosted CI proof must not be reported as native-device acceptance.
