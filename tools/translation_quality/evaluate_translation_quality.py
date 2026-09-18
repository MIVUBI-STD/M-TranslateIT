from __future__ import annotations

import argparse
import json
import re
import sys
import unicodedata
from collections import defaultdict
from pathlib import Path

SCHEMA = "translateit.translation_quality.v1"


def normalize(text: str) -> str:
    text = unicodedata.normalize("NFKC", text).casefold()
    text = re.sub(r"\s+", " ", text).strip()
    return text


def ngrams(text: str, n: int) -> dict[str, int]:
    compact = normalize(text)
    if len(compact) < n:
        return {compact: 1} if compact else {}
    counts: dict[str, int] = {}
    for i in range(len(compact) - n + 1):
        token = compact[i : i + n]
        counts[token] = counts.get(token, 0) + 1
    return counts


def char_ngram_f1(candidate: str, reference: str, max_n: int = 6) -> float:
    scores: list[float] = []
    for n in range(1, max_n + 1):
        cand = ngrams(candidate, n)
        ref = ngrams(reference, n)
        if not cand or not ref:
            continue
        overlap = sum(min(count, ref.get(token, 0)) for token, count in cand.items())
        precision = overlap / sum(cand.values())
        recall = overlap / sum(ref.values())
        scores.append(0.0 if precision + recall == 0 else 2 * precision * recall / (precision + recall))
    return sum(scores) / len(scores) if scores else 0.0


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
        if case.get("direction") not in {"id-en", "en-id"}:
            raise ValueError(f"{case_id}: unsupported direction")
        if not str(case.get("source", "")).strip():
            raise ValueError(f"{case_id}: empty source")
        refs = case.get("references")
        if not isinstance(refs, list) or not refs or not all(str(x).strip() for x in refs):
            raise ValueError(f"{case_id}: references required")
        for field in ("preserve", "required_any", "forbidden"):
            if field not in case:
                raise ValueError(f"{case_id}: missing {field}")
    return data


def invariant_result(case: dict, translated: str) -> dict:
    norm = normalize(translated)
    missing_literals = [item for item in case["preserve"] if normalize(item) not in norm]
    missing_concepts = [
        group for group in case["required_any"]
        if not any(normalize(option) in norm for option in group)
    ]
    forbidden_hits = [item for item in case["forbidden"] if normalize(item) in norm]
    return {
        "missing_literals": missing_literals,
        "missing_concepts": missing_concepts,
        "forbidden_hits": forbidden_hits,
        "critical_pass": not missing_literals and not missing_concepts and not forbidden_hits,
    }


def validate_corpus(corpus: dict) -> dict:
    failures: list[dict] = []
    for case in corpus["cases"]:
        if not any(invariant_result(case, ref)["critical_pass"] for ref in case["references"]):
            failures.append({"case_id": case["id"], "reason": "no reference satisfies declared invariants"})
    directions = sorted({case["direction"] for case in corpus["cases"]})
    categories = sorted({case["category"] for case in corpus["cases"]})
    return {"ok": not failures, "case_count": len(corpus["cases"]), "directions": directions, "categories": categories, "failures": failures}


def load_results(path: Path) -> dict[str, str]:
    raw = json.loads(path.read_text(encoding="utf-8"))
    rows = raw.get("results") if isinstance(raw, dict) else raw
    if not isinstance(rows, list):
        raise ValueError("results must be a list or an object containing results")
    output: dict[str, str] = {}
    for row in rows:
        case_id = str(row.get("case_id", "")).strip()
        translated = str(row.get("translated_text", "")).strip()
        if not case_id or case_id in output:
            raise ValueError(f"invalid or duplicate result case_id: {case_id!r}")
        output[case_id] = translated
    return output


def evaluate(corpus: dict, results: dict[str, str]) -> dict:
    expected = {case["id"] for case in corpus["cases"]}
    missing = sorted(expected - results.keys())
    unknown = sorted(results.keys() - expected)
    rows = []
    grouped: dict[str, list[float]] = defaultdict(list)
    critical_failures = 0
    for case in corpus["cases"]:
        translated = results.get(case["id"], "")
        inv = invariant_result(case, translated)
        score = max((char_ngram_f1(translated, ref) for ref in case["references"]), default=0.0)
        if not inv["critical_pass"]:
            critical_failures += 1
        grouped[case["direction"]].append(score)
        grouped[case["category"]].append(score)
        rows.append({"case_id": case["id"], "direction": case["direction"], "category": case["category"], "char_ngram_f1": round(score, 4), **inv})
    return {
        "schema": "translateit.translation_quality.report.v1",
        "complete_result_set": not missing and not unknown,
        "missing_case_ids": missing,
        "unknown_case_ids": unknown,
        "critical_failures": critical_failures,
        "critical_pass_rate": round((len(rows) - critical_failures) / len(rows), 4) if rows else 0.0,
        "mean_char_ngram_f1": round(sum(row["char_ngram_f1"] for row in rows) / len(rows), 4) if rows else 0.0,
        "group_means": {key: round(sum(values) / len(values), 4) for key, values in sorted(grouped.items())},
        "cases": rows,
        "note": "Character n-gram F1 and declared invariants are regression signals, not standalone proof of translation quality.",
    }


def compare_reports(corpus: dict, baseline_results: dict[str, str], candidate_results: dict[str, str]) -> dict:
    baseline = evaluate(corpus, baseline_results)
    candidate = evaluate(corpus, candidate_results)
    baseline_cases = {row["case_id"]: row for row in baseline["cases"]}
    candidate_cases = {row["case_id"]: row for row in candidate["cases"]}

    critical_regressions = []
    critical_recoveries = []
    score_deltas = {}
    for case_id in sorted(baseline_cases):
        base_row = baseline_cases[case_id]
        cand_row = candidate_cases[case_id]
        if base_row["critical_pass"] and not cand_row["critical_pass"]:
            critical_regressions.append(case_id)
        elif not base_row["critical_pass"] and cand_row["critical_pass"]:
            critical_recoveries.append(case_id)
        score_deltas[case_id] = round(
            cand_row["char_ngram_f1"] - base_row["char_ngram_f1"],
            4,
        )

    group_keys = sorted(set(baseline["group_means"]) | set(candidate["group_means"]))
    group_mean_deltas = {
        key: round(
            candidate["group_means"].get(key, 0.0) - baseline["group_means"].get(key, 0.0),
            4,
        )
        for key in group_keys
    }
    return {
        "schema": "translateit.translation_quality.comparison.v1",
        "complete_result_sets": (
            baseline["complete_result_set"] and candidate["complete_result_set"]
        ),
        "baseline_critical_failures": baseline["critical_failures"],
        "candidate_critical_failures": candidate["critical_failures"],
        "critical_regressions": critical_regressions,
        "critical_recoveries": critical_recoveries,
        "mean_char_ngram_f1_delta": round(
            candidate["mean_char_ngram_f1"] - baseline["mean_char_ngram_f1"],
            4,
        ),
        "group_mean_deltas": group_mean_deltas,
        "case_score_deltas": score_deltas,
        "promotion_safe_on_declared_critical_invariants": (
            baseline["complete_result_set"]
            and candidate["complete_result_set"]
            and not critical_regressions
        ),
        "note": (
            "Comparison detects newly introduced declared-invariant failures. "
            "It does not prove semantic superiority or replace human review."
        ),
    }


def emit_requests(corpus: dict) -> dict:
    requests = []
    for case in corpus["cases"]:
        source_language, target_language = case["direction"].split("-")
        requests.append({
            "case_id": case["id"],
            "request": {
                "command": "translate",
                "request_kind": "standalone_text",
                "text": case["source"],
                "source_language": source_language,
                "target_language": target_language,
            },
        })
    return {"schema": "translateit.translation_quality.requests.v1", "requests": requests}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "command",
        choices=("validate-corpus", "emit-requests", "evaluate", "compare"),
    )
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--results", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--candidate", type=Path)
    args = parser.parse_args()
    corpus = load_corpus(args.corpus)
    if args.command == "validate-corpus":
        report = validate_corpus(corpus)
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0 if report["ok"] else 1
    if args.command == "emit-requests":
        print(json.dumps(emit_requests(corpus), ensure_ascii=False, indent=2))
        return 0
    if args.command == "compare":
        if args.baseline is None or args.candidate is None:
            parser.error("--baseline and --candidate are required for compare")
        report = compare_reports(
            corpus,
            load_results(args.baseline),
            load_results(args.candidate),
        )
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0 if report["promotion_safe_on_declared_critical_invariants"] else 1

    if args.results is None:
        parser.error("--results is required for evaluate")
    report = evaluate(corpus, load_results(args.results))
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["complete_result_set"] and report["critical_failures"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
