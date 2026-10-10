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

## Active My Voice Quality Plan

Goal: improve voice resemblance, intelligibility, naturalness, and reliability using only existing GPT-SoVITS V2ProPlus. Voicebox is a research inspiration, not an approved second engine or proof of measured improvement. Product requirements PR-110 through PR-119 remain authoritative.

Evidence from current Local source:
- Guided take checks already validate WAV signal quality and known transcripts. Full training reference selection ranks eligible 3–10 second takes by clipping, silence, DC offset, duration proximity and line ID.
- Training creates bounded GPT/SoVITS checkpoint candidates; held-out synthesis is evaluated with artifact flags, intelligibility WER and speaker similarity. Candidate selection prioritizes clean audio and intelligibility before similarity, followed by user listening and approval.
- Quick Preview passes accepted candidates from Rust to Python and reuses existing full-training take signal admission plus reference ranking. Missing/corrupt/unusable WAVs are skipped; malformed/duplicate/unbounded metadata fails closed; existing evaluation artifact flags reject silent/clipped/dropout preview output. Source regression tests authored NOT EXECUTED; acoustic improvement unproven.
- Quick Preview source, WAV and quality gates, bounded stdin input, and release entrypoint are wired. Stale full-training status is excluded from preview phase reconciliation, and a bounded input writer is monitored for cancellation/timeout. UI invalidates previews after accepted recordings change or training begins. Source regression contracts authored NOT EXECUTED; compiler/model/Windows proof remain unknown.

Next action: conduct one admission review of the pinned integrated candidate against source/release contracts and `docs/knowledge/operations/target-windows-performance.md` A9. If no further source-confirmed blocker exists, reserve all unexecuted compiler, package, audio/GPU and Meeting checks for one grouped native acceptance; do not dispatch hosted CI or ask for incremental PC tests.

No benchmark improvement, native inference, installer validation, or runtime PASS is claimed. Branch Local remains development-only; main is reserved for expressly approved final results.
