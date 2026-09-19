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

## Release promotion gate

The Windows release build requires the aggregate readiness report explicitly:

```powershell
./EngineData/Frontend/RustApp/scripts/build_release.ps1 -QualityReadinessEvidence <quality-readiness-report.json>
```

The gate requires the report to target the exact committed release source identity, all three domains to be ready, no aggregate blockers, and valid domain report hashes. The report is not added to the Setup/Payload pair; only its SHA-256 and release identity are recorded in release build evidence.
