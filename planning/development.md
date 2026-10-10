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
- Quick Preview source, WAV/quality gates, bounded stdin input, status isolation and frontend freshness are wired. Source admission found the R3 Setup resource closure omitted both `voice_lab_quick_preview.py` and its required shared `voice_lab_gpt_sovits_build.py`; the Tauri resource map, exact release contract and required-file gate now include both. Static regression authored NOT EXECUTED; compiler/model/Windows proof remain unknown.

Next action: source packaging blocker closed by exact Local resource closure; conditionally prepare the one grouped LOCAL_CODE → TARGET_WINDOWS candidate gate in `docs/knowledge/operations/target-windows-performance.md` (including A9). Start with exact-SHA dependency, Node/Svelte/Python/Cargo verification, then create and validate offline Setup+Payload and install only if the preceding steps pass. Compiler/build/package/model/audio/GPU/Meeting proof remains NOT EXECUTED. No hosted CI, branch promotion or incremental user-PC tests.

No benchmark improvement, native inference, installer validation, or runtime PASS is claimed. Branch Local remains development-only; main is reserved for expressly approved final results.
