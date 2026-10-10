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
- Quick Preview now passes accepted candidate identities from Rust to the Python child, where existing training select_reference owns signal-first ranking. The new source and regression tests are NOT EXECUTED; acoustic improvement is unproven.
- Quick Preview source, cancellation guard, WAV validation and release entrypoint check are written. Native model output, build, and audio quality are NOT EXECUTED.

Next decisions in order:
1. Compare training and preview reference selection against actual user quality requirements. Reuse existing metrics and owners; consider changing ranking only after a concrete quality defect is established.
2. Review dataset preparation and candidate evaluation for demonstrated correctness gaps. Do not tune epochs or similarity cutoffs without comparative evidence.
3. Prepare reproducible listening and inference comparison across the same held-out English sentences, including speaker identity, pronunciation, artifact rate and stability. No new engine or separate score database.
4. Complete existing source/package contracts and enter one grouped Windows build/install/GPU/audio/Meeting acceptance only when integrated test-ready. Never run hosted CI/cloud inference or require incremental user-PC testing.

No benchmark improvement, native inference, installer validation, or runtime PASS is claimed. Branch Local remains development-only; main is reserved for expressly approved final results.
