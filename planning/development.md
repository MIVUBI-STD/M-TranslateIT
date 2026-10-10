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

Next: close the highest-priority source/integration proof gap through GitHub/cloud, then define the next evidence-grounded product change. `main` remains final-only and CI manual.

Verification definitions and source test entrypoints are available on `Local`; hosted dispatch is not yet established:

- All eight workflows are manual-only; ordinary commits do not trigger CI.
- Code Health manual dispatch = full-domain Frontend + Rust + Python proof with aggregate gate.
- Repository/quality/lock workflows write exact-SHA summaries.
- Manual R3 Release Contract requires controlled Windows payload proof.

If CI reports a concrete regression, repair only that regression.

Only after matching source/integration/package proof, assess the **integrated test-ready candidate** gate in `docs/knowledge/operations/target-windows-performance.md`. No per-change tests on the user's PC: plan one grouped `TARGET_WINDOWS` pass for installed-product, device, meeting, latency and clean-machine acceptance. Missing critical executable proof blocks candidate readiness; do not assign it to the user.

Source/hosted CI proof must not be reported as native-device acceptance.
