from __future__ import annotations

import argparse
import json
import re
import unicodedata
from collections import defaultdict
from pathlib import Path

SCHEMA = "translateit.asr_quality.v1"


def normalize(text: str) -> str:
    text = unicodedata.normalize("NFKC", text).casefold()
    text = re.sub(r"[^\w\s]+", " ", text, flags=re.UNICODE)
    return re.sub(r"\s+", " ", text).strip()


def edit_distance(reference: list[str], hypothesis: list[str]) -> int:
    previous = list(range(len(hypothesis) + 1))
    for i, ref_token in enumerate(reference, start=1):
        current = [i]
        for j, hyp_token in enumerate(hypothesis, start=1):
            substitution = previous[j - 1] + (ref_token != hyp_token)
            current.append(min(previous[j] + 1, current[j - 1] + 1, substitution))
        previous = current
    return previous[-1]


def error_rate(reference: list[str], hypothesis: list[str]) -> float:
    if not reference:
        return 0.0 if not hypothesis else 1.0
    return edit_distance(reference, hypothesis) / len(reference)


def word_error_rate(reference: str, hypothesis: str) -> float:
    return error_rate(normalize(reference).split(), normalize(hypothesis).split())


def char_error_rate(reference: str, hypothesis: str) -> float:
    return error_rate(list(normalize(reference).replace(" ", "")), list(normalize(hypothesis).replace(" ", "")))


def load_corpus(path: Path) -> dict:
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get("schema") != SCHEMA:
        raise ValueError(f"unsupported corpus schema: {data.get('schema')!r}")
    cases = data.get("cases")
    if not isinstance(cases, list) or not cases:
        raise ValueError("corpus must contain non-empty cases")
    seen: set[str] = set()
    for case in cases:
        case_id = str(case.get("id", "")).strip()
        if not case_id or case_id in seen:
            raise ValueError(f"invalid or duplicate case id: {case_id!r}")
        seen.add(case_id)
        if not str(case.get("category", "")).strip():
            raise ValueError(f"{case_id}: category required")
        if not str(case.get("recording_profile", "")).strip():
            raise ValueError(f"{case_id}: recording_profile required")
        if not str(case.get("reference", "")).strip():
            raise ValueError(f"{case_id}: reference required")
        for field in ("preserve", "required_any"):
            if field not in case or not isinstance(case[field], list):
                raise ValueError(f"{case_id}: {field} list required")
    return data


def invariant_result(case: dict, transcript: str) -> dict:
    norm = normalize(transcript)
    missing_literals = [item for item in case["preserve"] if normalize(item) not in norm]
    missing_concepts = [
        group for group in case["required_any"]
        if not any(normalize(option) in norm for option in group)
    ]
    return {
        "missing_literals": missing_literals,
        "missing_concepts": missing_concepts,
        "critical_pass": not missing_literals and not missing_concepts,
    }


def validate_corpus(corpus: dict) -> dict:
    failures = []
    for case in corpus["cases"]:
        inv = invariant_result(case, case["reference"])
        if not inv["critical_pass"]:
            failures.append({"case_id": case["id"], "reason": "reference violates declared invariants"})
    categories = sorted({case["category"] for case in corpus["cases"]})
    profiles = sorted({case["recording_profile"] for case in corpus["cases"]})
    return {
        "ok": not failures,
        "case_count": len(corpus["cases"]),
        "categories": categories,
        "recording_profiles": profiles,
        "failures": failures,
    }


def load_results(path: Path) -> dict[str, str]:
    raw = json.loads(path.read_text(encoding="utf-8"))
    rows = raw.get("results") if isinstance(raw, dict) else raw
    if not isinstance(rows, list):
        raise ValueError("results must be a list or an object containing results")
    output: dict[str, str] = {}
    for row in rows:
        case_id = str(row.get("case_id", "")).strip()
        transcript = str(row.get("transcript_text", "")).strip()
        if not case_id or case_id in output:
            raise ValueError(f"invalid or duplicate result case_id: {case_id!r}")
        output[case_id] = transcript
    return output


def evaluate(corpus: dict, results: dict[str, str]) -> dict:
    expected = {case["id"] for case in corpus["cases"]}
    missing = sorted(expected - results.keys())
    unknown = sorted(results.keys() - expected)
    rows = []
    grouped_wer: dict[str, list[float]] = defaultdict(list)
    grouped_cer: dict[str, list[float]] = defaultdict(list)
    critical_failures = 0
    for case in corpus["cases"]:
        transcript = results.get(case["id"], "")
        wer = word_error_rate(case["reference"], transcript)
        cer = char_error_rate(case["reference"], transcript)
        inv = invariant_result(case, transcript)
        if not inv["critical_pass"]:
            critical_failures += 1
        for key in (case["category"], case["recording_profile"]):
            grouped_wer[key].append(wer)
            grouped_cer[key].append(cer)
        rows.append({
            "case_id": case["id"],
            "category": case["category"],
            "recording_profile": case["recording_profile"],
            "wer": round(wer, 4),
            "cer": round(cer, 4),
            **inv,
        })
    count = len(rows)
    return {
        "schema": "translateit.asr_quality.report.v1",
        "complete_result_set": not missing and not unknown,
        "missing_case_ids": missing,
        "unknown_case_ids": unknown,
        "critical_failures": critical_failures,
        "critical_pass_rate": round((count - critical_failures) / count, 4) if count else 0.0,
        "mean_wer": round(sum(row["wer"] for row in rows) / count, 4) if count else 0.0,
        "mean_cer": round(sum(row["cer"] for row in rows) / count, 4) if count else 0.0,
        "group_wer": {key: round(sum(values) / len(values), 4) for key, values in sorted(grouped_wer.items())},
        "group_cer": {key: round(sum(values) / len(values), 4) for key, values in sorted(grouped_cer.items())},
        "cases": rows,
        "note": "WER/CER and declared invariants are regression evidence only for the exact evaluated audio, model, runtime and settings.",
    }


def fixture_plan(corpus: dict) -> dict:
    return {
        "schema": "translateit.asr_quality.fixture_plan.v1",
        "rules": [
            "Use authorized non-sensitive recordings only.",
            "Keep one utterance per WAV fixture.",
            "Do not normalize away the acoustic condition named by recording_profile.",
            "Record exact spoken wording from reference; punctuation is transcript metadata, not speech.",
            "Document microphone/device and source format with the captured result set.",
        ],
        "fixtures": [
            {
                "case_id": case["id"],
                "recording_profile": case["recording_profile"],
                "reference": case["reference"],
                "suggested_filename": f"{case['id']}.wav",
            }
            for case in corpus["cases"]
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("validate-corpus", "fixture-plan", "evaluate"))
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--results", type=Path)
    args = parser.parse_args()
    corpus = load_corpus(args.corpus)
    if args.command == "validate-corpus":
        report = validate_corpus(corpus)
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0 if report["ok"] else 1
    if args.command == "fixture-plan":
        print(json.dumps(fixture_plan(corpus), ensure_ascii=False, indent=2))
        return 0
    if args.results is None:
        parser.error("--results is required for evaluate")
    report = evaluate(corpus, load_results(args.results))
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["complete_result_set"] and report["critical_failures"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
