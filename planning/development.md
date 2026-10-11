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

## Active Translation & ASR Quality Continuation

Goal: improve quality truth using the existing Faster Whisper and MiLMMT pipeline rather than installing new models. The repository now carries three bounded improvements:
- The canonical ASR worker rejects transcripts longer than the existing text bound rather than silently emitting a truncated fragment to the Meeting translation/TTS pipeline.
- The existing ASR evaluator now exposes deterministic JiWER-style word substitution, deletion and insertion diagnostics plus word-weighted corpus WER. These are offline evaluation signals, not a second recognition engine or evidence of native ASR quality.
- The existing translation evaluator now localizes failures by declared invariant type and risk tag (XCOMET-inspired, not COMET model inference). Both evaluators avoid embedded-digit and embedded-negation false positives while preserving current intentionally stem-based Indonesian/decimal cues. Regression fixtures and runbook guidance are added without changing the pinned models or shipping new dependencies.
- Finalized Meeting outbound and optional incoming ASR now share one Rust-owned VAD policy: the native finalized utterance gate remains authoritative, and both subsequent Faster Whisper requests bypass its optional second Silero VAD pass. Other/raw ASR callers keep their worker-default behavior. No model, dependency, capture threshold, playback rule, or new runtime is introduced. This is SOURCE_WIRED only; speech retention, hallucinations, timing, and native behavior remain unmeasured.
- Native Meeting VAD now distinguishes clipping from silence while an utterance is active: clipped chunks remain bounded and retained for the unchanged whole-utterance validation instead of advancing end-of-speech. Clipping still cannot initiate speech or count toward accepted-speech duration. One existing Rust producer owner and its focused regression fixture cover the boundary; no denoiser, threshold, model, dependency, or product mode was added. SOURCE_WIRED; acoustic quality and TARGET_WINDOWS proof remain NOT EXECUTED.
- Short-utterance source review found separate duration admission criteria: the producer accepts sufficiently evidenced brief speech, while the finalized WAV writer imposes an independent minimum on retained frame length (which includes variable pre-roll/trailing silence). The canonical B2 scenario and existing TARGET_WINDOWS runbook now require owner-specific evidence for VAD non-finalization, writer `segment_too_short`, and downstream empty ASR. No thresholds, audio code, models or tests were changed; any policy correction is deferred until the single integrated native baseline proves the failure class.

Proof remains REMOTE_GITHUB static source review only; executable regression suites, GPU, real microphone, translation quality and Windows Meeting acceptance are NOT EXECUTED. Do not introduce neural metrics, external dataset copies or VAD swaps without a measured baseline. The existing combined target-Windows runbook remains the sole execution gate.

The existing incoming Meeting Sound consumer uses a bounded producer-condvar wait only while the canonical deferred queue has work; normal idle capture remains event-driven. Self-output suppression failure first marks the optional incoming lane disabled before clearing its producer/queue and stopping capture; late same-session status updates cannot restore eligibility. Both initial enqueue and front requeue now check existing Meeting session eligibility while holding the canonical deferred-queue mutex: a stale worker cannot repopulate the queue after Stop or suppression cleanup. The same bounded depth, oldest-drop behavior, original deferral age, priority and session reset remain. Source regression protects admission under the cleanup mutex and rejected post-clear retries. No new thread, scheduler, state store, dependency or polling service; Rust execution and TARGET_WINDOWS evidence remain NOT EXECUTED.

## Active Settings / Meeting Diagnostics Work

The existing Settings diagnostics owner now refreshes its Runtime Health panel on the same explicit Check Again action that refreshes the product snapshot; stale, superseded reads cannot overwrite the newest visible evidence. Unavailable watchdog/recovery states no longer display as Healthy or Clean, and diagnostic notes are rendered through the existing redaction utility. The existing Meeting Performance panel derives only an observed largest stage or queue/error guidance from existing runtime counters/timing, without inferring that stage is the root cause. The existing redacted support export now whitelists numeric per-stage timing to support reproducible bottleneck diagnosis, without copying user speech, exact utterance times or identifiers. No model, audio engine, new IPC command, monitoring timer or state store was added. Static contracts authored; actual Svelte/Rust compiler, native audio and GPU acceptance NOT EXECUTED.

## Active Text & Meeting Fidelity Work

The existing canonical MiLMMT worker remains the only translator. Source-level work now improves three existing boundaries without changing the model or adding a second runtime:
- Meeting outbound and optional incoming now require a successful, completed, EOS-terminated, canonical and direction-correct translation response in their shared Rust session owner before any TTS/commit. A contradictory worker/bridge success envelope is rejected rather than displayed or spoken.
- Text and Quick Translate now additionally verify source/target direction on the canonical paragraph-preserving result. On-demand Alternative Wording verifies its own complete canonical generation and direction before classifying a result as distinct or no-change; failed/incomplete outputs cannot masquerade as a harmless no-alternative response.
- Text review cues use existing simple word and phrase checks with punctuation-tolerant boundaries, allowing warnings for critical negations and corrections with punctuation while avoiding embedded-word false positives. No model-based UI review/scorer has been introduced.

Static source and Rust unit contracts were authored but NOT EXECUTED. The exact-SHA integrated test/build/package and native Meeting/Text acceptance remain the only proof path. Next priority is to measure end-to-end playback, latency and error types on target Windows rather than adding unmeasured architecture.

## Active My Voice Quality Plan

Goal: improve voice resemblance, intelligibility, naturalness, and reliability using only existing GPT-SoVITS V2ProPlus. Voicebox is a research inspiration, not an approved second engine or proof of measured improvement. Product requirements PR-110 through PR-119 remain authoritative.

Evidence from current Local source:
- Guided take checks already validate WAV signal quality and known transcripts. Full training reference selection ranks eligible 3–10 second takes by clipping, silence, DC offset, duration proximity and line ID.
- Training creates bounded GPT/SoVITS checkpoint candidates; held-out synthesis is evaluated with artifact flags, intelligibility WER and speaker similarity. Candidate selection prioritizes clean audio and intelligibility before similarity, followed by user listening and approval.
- Quick Preview passes accepted candidates from Rust to Python and reuses existing full-training take signal admission plus reference ranking. Missing/corrupt/unusable WAVs are skipped; malformed/duplicate/unbounded metadata fails closed; existing evaluation artifact flags reject silent/clipped/dropout preview output. Source regression tests authored NOT EXECUTED; acoustic improvement unproven.
- Held-out sample SHA-256 metadata now travels from the existing Python copy-and-hash validation through Rust evaluation DTO and typed frontend bridge. Explicit playback validates the returned bytes using Web Crypto before marking a line listened; failure is fail-closed, without new dependencies or a second engine. Rust validates the metadata format and canonical filename but does not yet recompute WAV SHA-256, so a backend-only integrity claim remains unproven. Python/Node/Rust regressions authored NOT EXECUTED.
- Review identity is now bound to the selected synthesis outputs (SHA-256 WAV identities) from the existing GPT-SoVITS evaluation. The Rust listener/approval contract rejects stale review identifiers; the frontend clears previous listening and quality confirmations across candidate changes. No second model or state owner. Cross-language regression authored NOT EXECUTED; installed/runtime and acoustic proof remain pending.
- Trained My Voice reviews are now source-bound to the exact frozen guided dataset. The existing evaluation owner compares accepted WAV bytes and dataset identities (not just length/timestamp) before surfacing, reading or promoting a trained candidate; changed/added/missing recordings fail closed, clear the displayed approval path, and require another build. Acceptance during active training is blocked; previously approved Meeting voices are unchanged. Rust regression tests authored NOT EXECUTED; no new engine, model, database, or dependency.
- Guided recording UI now uses the existing Rust signal-quality blocker reasons for specific corrective advice. Held-out My Voice approval requires explicit human clarity/naturalness/speaker-resemblance confirmation after listening; Rust enforces it before actor promotion. Rejecting a candidate leaves the existing Meeting voice unchanged; an alternative candidate uses the current build path. Source regression checks authored NOT EXECUTED; quality benefit requires real listening evidence.
- Quick Preview source, WAV/quality gates, bounded stdin input, status isolation and frontend freshness are wired. Source admission found the R3 Setup resource closure omitted both `voice_lab_quick_preview.py` and its required shared `voice_lab_gpt_sovits_build.py`; the Tauri resource map, exact release contract and required-file gate now include both. Static regression authored NOT EXECUTED; compiler/model/Windows proof remain unknown.

Next action: the canonical single integrated gate is now detailed in `docs/knowledge/operations/target-windows-performance.md` for Text, Meeting, Settings, My Voice and the offline R3 pair. The existing local Cargo helper now records a clean source SHA, rejects an explicitly pinned `TRANSLATEIT_EXPECTED_SHA` mismatch, always rebuilds frontend to avoid stale `dist`, and runs `cargo check --locked`. This is SOURCE_WIRED only; actual toolchain tests, release-quality evidence, signing, package creation, GPU/audio and installed Windows acceptance are NOT EXECUTED. Do not run hosted CI, promote `main`, fabricate a quality receipt, or request incremental user-PC testing.

No benchmark improvement, native inference, installer validation, or runtime PASS is claimed. Branch Local remains development-only; main is reserved for expressly approved final results.
