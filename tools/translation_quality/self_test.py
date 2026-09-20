from __future__ import annotations

import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EVALUATOR = ROOT / "evaluate_translation_quality.py"
CORPUS = ROOT / "corpus" / "translation_quality_v1.json"
HELDOUT = ROOT / "heldout" / "translation_quality_heldout_v1.json"


def load_evaluator():
    spec = importlib.util.spec_from_file_location("translateit_quality_evaluator", EVALUATOR)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    evaluator = load_evaluator()
    corpus = evaluator.load_corpus(CORPUS)
    validation = evaluator.validate_corpus(corpus)
    assert validation["ok"], validation
    assert validation["case_count"] >= 170
    assert set(validation["directions"]) == {"en-id", "id-en"}
    assert {"meaning", "number", "negation", "omission", "hallucination", "entity", "terminology"}.issubset(
        set(validation["risk_tags"])
    )

    heldout = evaluator.load_corpus(HELDOUT)
    heldout_validation = evaluator.validate_corpus(heldout)
    assert heldout_validation["ok"], heldout_validation
    assert heldout_validation["case_count"] == 40
    assert set(heldout_validation["directions"]) == {"en-id", "id-en"}
    assert {
        "prompt_boundary",
        "unicode",
        "repetition",
        "input_normalization",
        "contextual_meeting",
        "modality_uncertainty",
        "conditional_meaning",
        "quantifier_scope",
        "spoken_disfluency",
    }.issubset(set(validation["categories"]))

    fingerprint = evaluator.corpus_fingerprint(corpus)
    assert len(fingerprint) == 64

    perfect = {case["id"]: case["references"][0] for case in corpus["cases"]}
    report = evaluator.evaluate(corpus, perfect)
    assert report["complete_result_set"] is True
    assert report["critical_failures"] == 0
    assert report["critical_pass_rate"] == 1.0
    assert report["corpus_fingerprint"] == fingerprint
    assert report["provenance_matches_corpus"] is True
    assert report["group_critical_pass_rates"]["id-en"] == 1.0
    assert report["group_critical_pass_rates"]["contextual_meeting"] == 1.0
    assert report["risk_tag_critical_pass_rates"]["meaning"] == 1.0
    assert report["risk_tag_critical_pass_rates"]["number"] == 1.0

    mismatched = evaluator.evaluate(
        corpus,
        perfect,
        result_corpus_fingerprint="0" * 64,
        source_identity="candidate-sha",
    )
    assert mismatched["provenance_matches_corpus"] is False
    assert mismatched["complete_result_set"] is False

    bad = dict(perfect)
    bad["id-en-negation-001"] = "I will attend the meeting tomorrow."
    failed = evaluator.evaluate(corpus, bad)
    target = next(case for case in failed["cases"] if case["case_id"] == "id-en-negation-001")
    assert target["critical_pass"] is False
    assert target["forbidden_hits"]

    candidate = dict(perfect)
    candidate["id-en-negation-001"] = "I will attend the meeting tomorrow."
    comparison = evaluator.compare_reports(
        corpus,
        perfect,
        candidate,
        baseline_corpus_fingerprint=fingerprint,
        candidate_corpus_fingerprint=fingerprint,
        baseline_source_identity="baseline-sha",
        candidate_source_identity="candidate-sha",
    )
    assert comparison["complete_result_sets"] is True
    assert comparison["critical_regressions"] == ["id-en-negation-001"]
    assert comparison["critical_recoveries"] == []
    assert comparison["group_critical_pass_rate_deltas"]["negation"] < 0
    assert comparison["risk_tag_critical_pass_rate_deltas"]["negation"] < 0
    assert comparison["promotion_provenance_complete"] is True
    assert comparison["promotion_safe_on_declared_critical_invariants"] is False

    missing_provenance = evaluator.compare_reports(corpus, perfect, perfect)
    assert missing_provenance["promotion_provenance_complete"] is False
    assert missing_provenance["promotion_safe_on_declared_critical_invariants"] is False

    recovered = evaluator.compare_reports(
        corpus,
        candidate,
        perfect,
        baseline_corpus_fingerprint=fingerprint,
        candidate_corpus_fingerprint=fingerprint,
        baseline_source_identity="candidate-sha",
        candidate_source_identity="baseline-sha",
    )
    assert recovered["critical_regressions"] == []
    assert recovered["critical_recoveries"] == ["id-en-negation-001"]
    assert recovered["group_critical_pass_rate_deltas"]["negation"] > 0
    assert recovered["promotion_provenance_complete"] is True
    assert recovered["promotion_safe_on_declared_critical_invariants"] is True

    requests = evaluator.emit_requests(corpus)
    assert requests["corpus_fingerprint"] == fingerprint
    assert len(requests["requests"]) == validation["case_count"]
    contextual = [
        item
        for item in requests["requests"]
        if item["request"]["request_kind"] == "meeting_context_quality"
    ]
    standalone = [
        item
        for item in requests["requests"]
        if item["request"]["request_kind"] == "standalone_text"
    ]
    assert len(contextual) == 3
    assert standalone
    assert all(item["request"]["meeting_lane"] == "you" for item in contextual)
    assert all(item["request"]["meeting_generation"] == 1 for item in contextual)
    assert all(item["request"]["context_pairs"] for item in contextual)
    print(json.dumps({
        "ok": True,
        "regression_case_count": validation["case_count"],
        "heldout_case_count": heldout_validation["case_count"],
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
