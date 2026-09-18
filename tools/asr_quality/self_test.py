from __future__ import annotations

import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EVALUATOR = ROOT / "evaluate_asr_quality.py"
CORPUS = ROOT / "corpus" / "asr_quality_v1.json"


def load_evaluator():
    spec = importlib.util.spec_from_file_location("translateit_asr_quality_evaluator", EVALUATOR)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    evaluator = load_evaluator()
    corpus = evaluator.load_corpus(CORPUS)
    validation = evaluator.validate_corpus(corpus)
    assert validation["ok"], validation
    assert validation["case_count"] >= 18
    assert "code_switching" in validation["categories"]
    assert "moderate_room_noise" in validation["recording_profiles"]

    perfect = {case["id"]: case["reference"] for case in corpus["cases"]}
    report = evaluator.evaluate(corpus, perfect)
    assert report["complete_result_set"] is True
    assert report["critical_failures"] == 0
    assert report["mean_wer"] == 0.0
    assert report["mean_cer"] == 0.0

    assert evaluator.word_error_rate("saya tidak setuju", "saya setuju") > 0
    assert evaluator.char_error_rate("lima belas", "lima puluh") > 0

    bad = dict(perfect)
    bad["asr-negation-001"] = "Saya setuju dengan perubahan itu."
    failed = evaluator.evaluate(corpus, bad)
    target = next(case for case in failed["cases"] if case["case_id"] == "asr-negation-001")
    assert target["critical_pass"] is False
    assert target["missing_concepts"]

    plan = evaluator.fixture_plan(corpus)
    assert len(plan["fixtures"]) == validation["case_count"]
    assert all(item["suggested_filename"].endswith(".wav") for item in plan["fixtures"])
    print(json.dumps({"ok": True, "case_count": validation["case_count"]}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
