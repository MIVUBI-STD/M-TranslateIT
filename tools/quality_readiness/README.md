# Quality Readiness

This directory aggregates existing Translation, ASR, and TTS/My Voice **comparison reports** into one release-quality evidence result.

It deliberately does not reimplement domain metrics. Translation owns semantic regression, ASR owns recognition/acoustic regression, and TTS owns speaker/intelligibility/artifact regression.

## Manifest

Create a local manifest beside the three comparison reports:

```json
{
  "schema": "translateit.quality_readiness.manifest.v1",
  "release_identity": "<exact release/source identity>",
  "domains": {
    "translation": {
      "report": "translation-comparison.json",
      "expected_candidate_source_identity": "<translation candidate identity>"
    },
    "asr": {
      "report": "asr-comparison.json",
      "expected_candidate_source_identity": "<ASR candidate identity>"
    },
    "tts": {
      "report": "tts-comparison.json",
      "expected_candidate_source_identity": "<voice/runtime candidate identity>"
    }
  }
}
```

Each report filename must be local to the manifest directory. The aggregator records the SHA-256 of every input report.

## Run

```powershell
python tools/quality_readiness/aggregate_quality_readiness.py --manifest <manifest.json>
```

The aggregate result is ready only when all three domains:

- contain complete matched result sets;
- have complete promotion provenance;
- match the candidate identity declared by the manifest;
- pass their own fail-closed critical-regression gate.

This is a release **evidence aggregation** gate, not a runtime pipeline and not native acceptance. TARGET_WINDOWS remains authoritative for physical microphone behavior, meeting routing/reception, practical latency, and audible listening quality.

## Optional end-to-end Meeting evidence (Q2)

The original three-domain manifest remains valid with no changes. For integrated
speech-to-speech evaluation, optionally add to the same manifest:

```json
"meeting_trace": {
  "report": "meeting-trace.json",
  "expected_case_ids": ["negation-001"]
}
```

The referenced **local-only** JSON report has schema
`translateit.meeting_fidelity.trace.v1` and this shape:

```json
{
  "schema": "translateit.meeting_fidelity.trace.v1",
  "source_identity": "<exact release/source identity>",
  "domain_source_identities": {
    "translation": "<candidate identity from translation comparison>",
    "asr": "<candidate identity from ASR comparison>",
    "tts": "<candidate identity from TTS comparison>"
  },
  "cases": [{
    "case_id": "negation-001",
    "risk_tags": ["negation", "correction"],
    "source_wav_sha256": "<sha256 of exact captured source audio>",
    "asr": {
      "audio_sha256": "<same captured source WAV hash>",
      "transcript_sha256": "<sha256 of exact ASR transcript bytes>",
      "status": "complete"
    },
    "translation": {
      "input_transcript_sha256": "<same ASR transcript hash>",
      "output_text_sha256": "<sha256 of exact translated text bytes>",
      "direction": "id-en",
      "complete": true
    },
    "tts": {
      "input_text_sha256": "<same translated text hash>",
      "wav_sha256": "<sha256 of exact synthesized WAV>",
      "status": "complete"
    },
    "delivery": {
      "wav_sha256": "<same synthesized WAV hash>",
      "status": "output_complete"
    },
    "meaning_review": {"verdict": "no_critical_error", "reviewer_count": 1}
  }]
}
```

Use real stage outputs captured from the **same utterance and runtime
identity**. Hash exact bytes at the owning stage; hashing rules and evidence
capture must be consistent across stages. This tool cannot verify that a
self-declared hash or review verdict truly came from running the models.
The test fixture only exercises the metadata contract—it is **not evidence of
accuracy**. Before any release-quality claim, retain the actual local,
independently reviewed evidence as required by the human evaluation protocol.

When enabled, the existing aggregator checks source/model identity, exactly
the declared case set, valid SHA-256 representation, contiguous stage hashes,
successful stage outcomes, and explicit no-critical-error bilingual review.
Missing cases, mismatched identities/hashes, unreviewed or critically incorrect
outputs fail the optional trace gate. The aggregate records a hash of the
receipt and the case count without copying private speech or transcript text.
The release validator also refuses any explicitly present failed trace.

**Privacy:** do not put WAVs, transcripts, raw utterances, speaker
identifiers, reviewer names, or private paths into tracked Git or into the trace
report. Case IDs and risk tags must remain nonsensitive slugs. The report
accepts only metadata fields, up to 64 cases, and a 2 MiB size limit. A
matched hash does **not** prove Windows device routing, acoustic fidelity,
human listening, or meeting-app reception. These remain TARGET_WINDOWS work.

## Release promotion gate

The Windows release build requires the aggregate readiness report explicitly:

```powershell
./EngineData/Frontend/RustApp/scripts/build_release.ps1 -QualityReadinessEvidence <quality-readiness-report.json>
```

The gate requires the report to target the exact committed release source identity, all three domains to be ready, no aggregate blockers, and valid domain report hashes. The report is not added to the Setup/Payload pair; only its SHA-256 and release identity are recorded in release build evidence.
