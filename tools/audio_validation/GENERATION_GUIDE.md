# WAV generation and reference profiles

The WAV generator is **developer tooling**. No meeting microphone, online TTS, model inference, CI, user account or voice cloning is required. All sample texts are public and already registered in the canonical 48-case suite; the existing 60-case ASR corpus remains the sole owner of its referenced sentences.

## Offline generation pipeline

1. Inspect `tools/audio_validation/scenarios.json` and `tools/asr_quality/corpus/asr_quality_v1.json`. Each A01–A48 points to one public source or a local literal.
2. Use a locally installed reviewed **eSpeak-compatible executable**. Validate its installed `id` and `en` voices independently. Pin its full executable path and version; never download a model implicitly.
3. Each speech fixture is synthesized with the corresponding source language, then converted to canonical 16,000 Hz mono PCM16 WAV. The same seed applies deterministic configured volume, clip, additive white noise, silence and pause perturbations. The three non-speech fixtures are generated without TTS: A13 silence, A14 seeded noise, A15 a continuous 220 Hz tone.
4. Each WAV is inspected for rate, channels, frame duration, RMS and peak. The official generator emits `fixtures.json` with each audio hash, text-reference SHA, scenario/corpus fingerprint and synthesizer version. Never overwrite a previously generated fixture directory. Keep it under ignored `UserData/CacheData/audio_validation/<unique-id>`.
5. Run `harness.py inspect` to verify **all** sample identities. The evaluator rejects extra files, changed WAV bytes and unmatched scenarios. Only these official fixtures are eligible for canonical Rust replay and real local-worker tests.

```powershell
python tools/audio_validation/harness.py validate
python tools/audio_validation/generation_doctor.py --tts-exe espeak-ng
python tools/audio_validation/harness.py generate --tts-exe espeak-ng --seed 42 --fixtures UserData/CacheData/audio_validation/espeak-id-en-v1
python tools/audio_validation/harness.py inspect --fixtures UserData/CacheData/audio_validation/espeak-id-en-v1
```

On a machine with the `espeak` binary instead of `espeak-ng`, use `--tts-exe espeak` in both commands. The `generation_doctor.py` command is observational only. If the required voice is unavailable, generation **fails** rather than silently selecting another language.

## Audio sources / future extensions

| Tier | Source | Appropriate use | Does not prove |
|---|---|---|---|
| S0 | eSpeak offline, fixed text/voice/version | Contract smoke, timing/control, deterministic perturbations | Natural human ASR quality |
| S1 | Locally approved natural TTS with an explicit pinned license, voice and version | More realistic public synthetic voices, varied timbre and duration | Real human accents, microphone conditions |
| R1 | Separately licensed, consented or public human recordings with known transcripts | ASR pronunciation/accent and spontaneous utterance benchmark | Windows device/output route |
| W1 | Synthetic WAV fed into authorized native Windows virtual source and independently captured receiver | Native output-route and lifecycle acceptance | Physical mic quality in every environment |

Do not claim that a noise or gain filter reproduces `Zoom/Teams/Meet` codecs, real voice overlap, Bluetooth profiles or local Indonesian accents. Those require actual acoustic/source evidence. Store these categories separately and never compare two generator versions or voices as if they were the same fixture.

## Recommended coverage profile

- **48 base WAVs:** exact canonical suite for immediate replay and worker smoke. The majority are Indonesian with one explicit English incoming sample.
- **Acoustic variants:** produce *derived* test runs with known new hashes and declared transformations rather than multiplying committed WAV files. Retain an unchanged baseline.
- **Speaker coverage:** add a separate versioned voice matrix later (male/female, natural fast/slow, accents) only with genuinely different approved speech sources. Do not emulate accents solely by editing metadata.
- **Reference text:** the transcript is the intended spoken script, not proof that eSpeak pronounced code-switching, names, letters and numbers perfectly. ASR reference mismatches may come from the synthesizer and must be listened to/reviewed on public fixtures before quality thresholds are trusted.

## Preview package boundary

The independent `translateit_audio_preview_48.zip` supplied in chat is a convenience **PREVIEW** generated with the installed offline `espeak` executable, 48 public texts and deterministic source processing. It includes a preview manifest (not the official `translateit.audio_fixtures.v1` generator manifest) and is **not** committed into the GitHub repository. The official fixtures are generated via `harness.py generate` and their hashes are newly derived at that time. No claim is made that ASR, VAD, MiLMMT, TTS or Meeting playback passed.
