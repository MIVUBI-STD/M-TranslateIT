from __future__ import annotations

import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EVALUATOR = ROOT / "evaluate_tts_quality.py"
CORPUS = ROOT / "corpus" / "tts_quality_v1.json"


def load_evaluator():
    spec = importlib.util.spec_from_file_location("translateit_tts_quality_evaluator", EVALUATOR)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def evidence(corpus: dict, similarity: float = 0.9) -> dict:
    return {
        case["id"]: {
            "intelligibility_text": case["text"],
            "speaker_similarity": similarity,
            "artifact_flags": [],
            "wav_sha256": "a" * 64,
        }
        for case in corpus["cases"]
    }


def main() -> int:
    evaluator = load_evaluator()
    corpus = evaluator.load_corpus(CORPUS)
    validation = evaluator.validate_corpus(corpus)
    assert validation["ok"], validation
    assert validation["case_count"] == 10
    assert {"meeting_core", "technical", "long_form", "prosody_challenge"}.issubset(
        set(validation["categories"])
    )

    fingerprint = evaluator.corpus_fingerprint(corpus)
    perfect = evidence(corpus)
    report = evaluator.evaluate(
        corpus,
        perfect,
        result_corpus_fingerprint=fingerprint,
        source_identity="baseline-sha",
    )
    assert report["complete_result_set"] is True
    assert report["critical_failures"] == 0
    assert report["mean_intelligibility_wer"] == 0.0
    assert report["mean_speaker_similarity"] == 0.9

    candidate = evidence(corpus, 0.92)
    candidate["tts-negation-001"]["intelligibility_text"] = (
        "I approved the payment, so please send it."
    )
    candidate["tts-negation-001"]["artifact_flags"] = ["truncation"]
    comparison = evaluator.compare_reports(
        corpus,
        perfect,
        candidate,
        baseline_corpus_fingerprint=fingerprint,
        candidate_corpus_fingerprint=fingerprint,
        baseline_source_identity="baseline-sha",
        candidate_source_identity="candidate-sha",
    )
    assert comparison["mean_speaker_similarity_delta"] > 0
    assert comparison["critical_regressions"] == ["tts-negation-001"]
    assert comparison["artifact_regressions"] == ["tts-negation-001"]
    assert comparison["promotion_safe_on_declared_critical_invariants"] is False

    missing_provenance = evaluator.compare_reports(corpus, perfect, perfect)
    assert missing_provenance["promotion_provenance_complete"] is False
    assert missing_provenance["promotion_safe_on_declared_critical_invariants"] is False

    plan = evaluator.fixture_plan(corpus)
    assert plan["corpus_fingerprint"] == fingerprint
    assert len(plan["fixtures"]) == validation["case_count"]
    print(json.dumps({"ok": True, "case_count": validation["case_count"]}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
