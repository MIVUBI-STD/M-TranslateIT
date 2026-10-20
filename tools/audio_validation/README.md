# Offline Audio Regression Harness (developer tool)

**Scope:** Reproducible public audio fixtures and evidence-scoped regression checks. This is **not** a new production VAD, ASR, translator, or TTS engine. The 60-case corpus and all canonical ASR reference/invariant logic remain in `tools/asr_quality/`. This harness selects 48 versioned scenarios and adds audio-file provenance.

## Safety / execution boundary

- **GitHub CI is paused by owner.** No GitHub Actions job, model inference, Windows microphone, device capture or cloud service runs automatically.
- `harness.py validate/generate/inspect/score/compare` never starts a model or microphone. `generate` only runs an explicitly supplied installed offline **eSpeak-compatible** executable. Check its license and language voices in your environment.
- `worker_runner.py` is **explicit opt-in**, requires a clean checkout at an externally pinned `Local` SHA and an existing pinned WorkerRuntime Python interpreter/model assets. It starts only the canonical persistent worker, with forced offline Hugging Face flags. It **does not** run Meeting, native VAD, virtual microphone, speaker playback, receiver delivery or training.
- Use **public test utterances** only. Do not commit user speech, private recordings, selected personal-voice data, model binaries, or generated WAVs. Use `UserData/CacheData/audio_validation/<unique-run>` for generated fixtures and `UserData/LogData/` for local observations.
- The test-only Rust PCM/VAD replay exists at `EngineData/Frontend/RustApp/src-tauri/src/engine/audio/pcm_handoff_replay.rs`. It has **no production capture cutover**. No direct worker smoke can be counted as VAD/meeting-route acceptance.

## Workflow

From repository root, with Python 3.12 and an installed offline eSpeak-compatible synthesizer:

```powershell
python tools/audio_validation/harness.py validate
python tools/audio_validation/harness.py generate --tts-exe espeak-ng --fixtures UserData/CacheData/audio_validation/public-v1
python tools/audio_validation/harness.py inspect --fixtures UserData/CacheData/audio_validation/public-v1
```

Use `espeak` instead of `espeak-ng` **only** if it is the explicitly installed, reviewed offline executable supporting `id` and `en`. Fixture generation records the version, deterministic seed, canonical corpus/scenario fingerprint, SHA-256 of reference text and every WAV, plus PCM metrics. It refuses to overwrite prior output. The deterministic linear resampler is solely for smoke-fixture conversion, **not** an audio-quality benchmark of the application.

To exercise the existing AI worker on the user's **already prepared** local Windows test environment after explicit acceptance authorization (no downloads, no microphone), supply the pinned interpreter and SHA:

```powershell
python tools/audio_validation/worker_runner.py --fixtures UserData/CacheData/audio_validation/public-v1 --python EngineData/Backend/LocalWorker/WorkerRuntime/.venv/Scripts/python.exe --source-sha <40-char-clean-Local-SHA> --ids A01 A03 A04 A12 --output UserData/LogData/audio_validation-worker-v1.json
python tools/audio_validation/harness.py score --fixtures UserData/CacheData/audio_validation/public-v1 --observations UserData/LogData/audio_validation-worker-v1.json
```

Add `--include-voice` only to run selected voice synthesis; it does not replace actual receiver listening checks. `score` refuses unbound audio hashes, missing scenario coverage, mismatched source/scenario fingerprints, wrong translation direction, missing EOS, invalid TTS receipts and declared ASR critical cue regressions. A `PASS` label applies only to the *named observed stages*; the report always records the explicit proof ceiling. `vad_asr`, `no_speech` and `numeric_overflow` remain **UNVERIFIED** until a real offline Rust VAD/WAV-writer replay receipt exists.

To exercise the **actual canonical Rust finalizer and PCM16 writer**, without starting a microphone or changing production capture, use the explicit ignored Rust test via the pinned local runner. This requires the usual existing locked Cargo build prerequisites and a clean `Local` checkout; it is **not CI**:

```powershell
python tools/audio_validation/vad_runner.py --fixtures UserData/CacheData/audio_validation/public-v1 --source-sha <40-char-clean-Local-SHA> --output UserData/LogData/audio-validation-vad-v1.json
python tools/audio_validation/harness.py score --fixtures UserData/CacheData/audio_validation/public-v1 --observations UserData/LogData/audio-validation-vad-v1.json
```

To **combine Rust VAD evidence and real local worker output** from the exact same source SHA and byte-identical fixtures, set `--worker-observations UserData/LogData/audio_validation-worker-v1.json` and a new `--output`. Only the selected direct worker stages from the prior receipt and observed Rust VAD stages are combined; the test runner refuses a duplicate VAD receipt or mismatched identities. It never upgrades a test-only incoming finalizer into a physical mic/Meeting receiver claim. The offline replay adds an explicitly reported one-second quiet tail to finalize incomplete speech, but does not add one to A15's 61-second uninterrupted numeric stress tone.

The Rust test has `#[ignore]` and therefore does not run in routine Rust tests. Its receipts include the canonical incoming producer's finalized/drop/overflow counts and the canonical PCM16 writer outcome. It **does not** exercise CPAL input device acquisition, session/generation-based outbound routing, the production writer's complete path admission, or user speech quality. Unselected stages stay `UNVERIFIED`. If actual speech is misclassified by the existing gate, preserve that failure as a diagnostic rather than weakening the gate to force a green result.

Compare results from an identical fixture set:

```powershell
python tools/audio_validation/harness.py compare --baseline <scored-baseline.json> --candidate <scored-candidate.json>
```

A change of generator, WAV bytes, corpus or scenario version blocks a direct comparison rather than silently presenting unrelated measurements as an improvement.

## Proof matrix (strict)

| Layer | Source of evidence | Current status |
|---|---|---|
| L0 | Schema and case validation | Source implemented; must be executed to claim PASS |
| L1 | PCM16 mono 16-kHz WAV, SHA-256 fixture identity | Explicit offline generation/inspection |
| L2 | Canonical Rust incoming VAD/finalizer and PCM16 writer | Explicit ignored Rust test + `vad_runner.py`; no native Meeting proof |
| L3 | Real pinned worker ASR → MiLMMT → optional selected TTS | Explicit local `worker_runner.py`; never automatic |
| L4 | Windows virtual route → separate actual meeting-app receiver | NOT IMPLEMENTED / NOT EXECUTED in harness |
| L5 | Real physical microphone and ambient conditions | Native/manual acceptance still required |

Numeric 61-second tone A15 tests a buffer/overflow **boundary only**, never natural continuous-speech ASR quality. ASR TTS synthesized read speech does not prove human accents or spontaneous Meeting comprehension. No green source test overrides these distinctions.

## Extension rules

Add scenarios to `scenarios.json` with a unique ID and explicit expectation; prefer referencing `tools/asr_quality/corpus/asr_quality_v1.json` instead of duplicating speech text. Keep the generator deterministic, provenance append-only, outputs bounded and private, and model/receiver proof independently labeled. Do not inject test audio into the production capture path or change VAD thresholds simply to pass synthetic sound fixtures. L4 automation still requires separate target-lane ownership and receiver/device evidence review.

## Deep test strategy

The full G0–G5 proof ladder, scenario boundaries and 28 planned fault cases are documented in [DEEP_ACCEPTANCE.md](DEEP_ACCEPTANCE.md) and [fault_matrix.json](fault_matrix.json). These are test specifications, not executed proof. The original 16-WAV preview is not the expanded 48-case canonical fixture set. Only generate the new set on explicit local tooling execution, preserving SHA/model identity.
