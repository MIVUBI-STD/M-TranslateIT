from __future__ import annotations

import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EVALUATOR = ROOT / "evaluate_translation_quality.py"
CORPUS = ROOT / "corpus" / "translation_quality_v1.json"


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
    assert validation["case_count"] >= 16
    assert set(validation["directions"]) == {"en-id", "id-en"}

    perfect = {case["id"]: case["references"][0] for case in corpus["cases"]}
    report = evaluator.evaluate(corpus, perfect)
    assert report["complete_result_set"] is True
    assert report["critical_failures"] == 0
    assert report["critical_pass_rate"] == 1.0

    bad = dict(perfect)
    bad["id-en-negation-001"] = "I will attend the meeting tomorrow."
    failed = evaluator.evaluate(corpus, bad)
    target = next(case for case in failed["cases"] if case["case_id"] == "id-en-negation-001")
    assert target["critical_pass"] is False
    assert target["forbidden_hits"]

    requests = evaluator.emit_requests(corpus)
    assert len(requests["requests"]) == validation["case_count"]
    assert all(item["request"]["request_kind"] == "standalone_text" for item in requests["requests"])
    print(json.dumps({"ok": True, "case_count": validation["case_count"]}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
