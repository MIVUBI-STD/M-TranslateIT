# ASR Quality

This directory owns regression-evaluation infrastructure for TranslateIT's canonical Indonesian Meeting ASR path.

It does **not** contain fabricated audio or claim that faster-whisper is accurate because CI is green. Hosted CI validates the specification and evaluator. Real ASR quality requires exact audio fixtures to be processed by the canonical worker/model/runtime.

## Corpus

`corpus/asr_quality_v1.json` covers high-risk meeting speech:

- negation and corrections;
- numbers, dates, times and spoken versions;
- technical vocabulary;
- Indonesian/English code-switching;
- names and identifiers;
- natural pauses and filler speech;
- soft/fast speech;
- moderate room noise and room echo;
- far-field room capture, fan/keyboard noise and light crosstalk;
- mild clipping and low-gain speech;
- Bluetooth headset and built-in laptop microphone profiles.

`recording_profile` describes the intended acoustic condition. It is part of the fixture identity and must not be silently normalized away when real audio is captured.

## Metrics

The evaluator reports:

- word error rate (WER), both per-utterance mean and word-weighted corpus WER;
- word-level substitution, deletion and insertion counts (JiWER-style edit analysis using the existing evaluator, without an additional engine);
- character error rate (CER);
- critical literal/concept preservation;
- grouped WER/CER by linguistic category and recording profile;
- grouped critical-invariant pass rates;
- baseline-vs-candidate critical regression/recovery deltas.

Deletion spikes may indicate dropped/shortened speech; insertion spikes may indicate spurious output. These are diagnostic clues, not causal proof. The worker now fails explicitly on overlong finalized ASR text instead of silently truncating text passed to translation. All of this still requires executed audio evidence.

External benchmark direction: [Common Voice dataset metadata](https://github.com/common-voice/cv-dataset) can guide a separately licensed Indonesian read-speech fixture set; do not put downloaded voice clips or private recordings in Git or treat read speech as representative of spontaneous meetings. Retain exact dataset release, transcript and audio identity in the native benchmark receipts. This is a source-preparation direction, not an imported or executed corpus.

WER/CER are useful regression metrics, not a standalone user-quality verdict. A transcript can have a moderate WER yet still contain a critical reversal such as losing the word "tidak", so critical invariants are evaluated separately.

## Commands

Validate the corpus:

```powershell
python tools/asr_quality/evaluate_asr_quality.py validate-corpus --corpus tools/asr_quality/corpus/asr_quality_v1.json
```

Generate the recording/fixture plan:

```powershell
python tools/asr_quality/evaluate_asr_quality.py fixture-plan --corpus tools/asr_quality/corpus/asr_quality_v1.json
```

Evaluate captured ASR transcripts:

```powershell
python tools/asr_quality/evaluate_asr_quality.py evaluate --corpus tools/asr_quality/corpus/asr_quality_v1.json --results <results.json>
```

Expected result format:

```json
{
  "corpus_fingerprint": "<sha256 from fixture-plan>",
  "source_identity": "<exact Local/model/settings/runtime identity>",
  "results": [
    {"case_id": "asr-negation-001", "transcript_text": "..."}
  ]
}
```

Compare matched baseline and candidate captures:

```powershell
python tools/asr_quality/evaluate_asr_quality.py compare --corpus tools/asr_quality/corpus/asr_quality_v1.json --baseline <baseline.json> --candidate <candidate.json>
```

Promotion comparison requires both result sets to identify their source and match the same corpus fingerprint. This prevents WER/CER improvements from being compared across silently different fixture specifications.

## Evidence boundary

A valid ASR comparison records at minimum the exact `Local` SHA, model ID, device/compute type, beam size, VAD setting, microphone/source, and the exact fixture set. Compare candidates on the same fixtures.

Do not change Whisper model, beam, VAD or segmentation based only on repository tests. Route the first reproducible quality or latency defect back to the owning runtime after real evidence exists.


## Expanded native acceptance matrix

The 60-case specification now includes target profiles for Javanese- and Sundanese-accented Indonesian, Jakarta casual speech, formal presentation speech, Zoom/Teams/Google Meet compression, USB/webcam/speakerphone devices, fast code-switching, long utterances, light overlap, packetized meeting audio, acronyms and list/order language.

These are recording targets, not CI claims. A profile becomes evidence only after real audio is captured under the declared condition and evaluated with exact runtime provenance.
