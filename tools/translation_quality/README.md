# Translation Quality

This directory owns source-level translation quality evaluation infrastructure for the canonical Indonesian ↔ English pipeline.

It deliberately separates **quality measurement** from **quality claims**:

- hosted CI validates corpus integrity and evaluator behavior;
- real MiLMMT output must be produced by a runtime with the canonical model installed;
- no repository/static result is reported as proof of model quality.

## Corpus

`corpus/translation_quality_v1.json` is a versioned regression set covering both directions and high-risk meeting cases:

- negation and corrections;
- numbers, dates and units;
- URLs, IDs and names;
- technical vocabulary;
- Indonesian/English code-switching;
- meeting language.

Each case contains one or more references plus narrow invariants:

- `preserve` — literals that must survive;
- `required_any` — semantic cue groups where at least one form must appear;
- `forbidden` — high-confidence meaning reversals or unsafe phrases.

These checks are intentionally narrow. They do not replace human review.

## Commands

Validate corpus integrity:

```powershell
python tools/translation_quality/evaluate_translation_quality.py validate-corpus --corpus tools/translation_quality/corpus/translation_quality_v1.json
```

Emit canonical worker requests:

```powershell
python tools/translation_quality/evaluate_translation_quality.py emit-requests --corpus tools/translation_quality/corpus/translation_quality_v1.json
```

Evaluate captured results:

```powershell
python tools/translation_quality/evaluate_translation_quality.py evaluate --corpus tools/translation_quality/corpus/translation_quality_v1.json --results <results.json>
```

Result format:

```json
{
  "results": [
    {"case_id": "id-en-negation-001", "translated_text": "..."}
  ]
}
```

The report includes critical invariant pass rate, a lightweight character n-gram F1 regression signal, direction/category means, and per-case failures. The n-gram score is not BLEU, COMET, or a substitute for linguistic evaluation.

## Promotion rule

Do not change model, prompt, context policy, decoding policy, or text segmentation based on one anecdotal sentence. Capture a baseline from the current canonical model, compare the candidate on the same corpus, inspect every critical failure, and then add a new case when a real recurring failure mode is discovered.

Do not tune the corpus to make a candidate look better.
