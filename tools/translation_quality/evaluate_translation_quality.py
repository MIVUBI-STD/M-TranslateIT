from __future__ import annotations

import argparse
import hashlib
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


def corpus_fingerprint(corpus: dict) -> str:
    canonical = json.dumps(
        corpus,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(canonical).hexdigest()


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
        context_pairs = case.get("context_pairs")
        if context_pairs is not None:
            if case["direction"] != "id-en":
                raise ValueError(f"{case_id}: contextual evaluation is outbound id-en only")
            if (
                not isinstance(context_pairs, list)
                or not context_pairs
                or len(context_pairs) > 3
            ):
                raise ValueError(f"{case_id}: context_pairs must contain one to three pairs")
            for pair in context_pairs:
                if (
                    not isinstance(pair, list)
                    or len(pair) != 2
                    or not all(isinstance(value, str) and value.strip() for value in pair)
                ):
                    raise ValueError(f"{case_id}: invalid context pair")
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


def load_result_bundle(path: Path) -> dict:
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
    return {
        "results": output,
        "corpus_fingerprint": (
            str(raw.get("corpus_fingerprint", "")).strip()
            if isinstance(raw, dict)
            else ""
        ),
        "source_identity": (
            str(raw.get("source_identity", "")).strip()
            if isinstance(raw, dict)
            else ""
        ),
    }


def load_results(path: Path) -> dict[str, str]:
    return load_result_bundle(path)["results"]


def evaluate(
    corpus: dict,
    results: dict[str, str],
    *,
    result_corpus_fingerprint: str = "",
    source_identity: str = "",
) -> dict:
    expected = {case["id"] for case in corpus["cases"]}
    missing = sorted(expected - results.keys())
    unknown = sorted(results.keys() - expected)
    rows = []
    grouped: dict[str, list[float]] = defaultdict(list)
    grouped_critical: dict[str, list[bool]] = defaultdict(list)
    critical_failures = 0
    for case in corpus["cases"]:
        translated = results.get(case["id"], "")
        inv = invariant_result(case, translated)
        score = max((char_ngram_f1(translated, ref) for ref in case["references"]), default=0.0)
        if not inv["critical_pass"]:
            critical_failures += 1
        grouped[case["direction"]].append(score)
        grouped[case["category"]].append(score)
        grouped_critical[case["direction"]].append(inv["critical_pass"])
        grouped_critical[case["category"]].append(inv["critical_pass"])
        rows.append({"case_id": case["id"], "direction": case["direction"], "category": case["category"], "char_ngram_f1": round(score, 4), **inv})
    expected_fingerprint = corpus_fingerprint(corpus)
    provenance_matches = (
        not result_corpus_fingerprint
        or result_corpus_fingerprint == expected_fingerprint
    )
    return {
        "schema": "translateit.translation_quality.report.v1",
        "corpus_fingerprint": expected_fingerprint,
        "result_corpus_fingerprint": result_corpus_fingerprint or None,
        "source_identity": source_identity or None,
        "provenance_matches_corpus": provenance_matches,
        "complete_result_set": not missing and not unknown and provenance_matches,
        "missing_case_ids": missing,
        "unknown_case_ids": unknown,
        "critical_failures": critical_failures,
        "critical_pass_rate": round((len(rows) - critical_failures) / len(rows), 4) if rows else 0.0,
        "mean_char_ngram_f1": round(sum(row["char_ngram_f1"] for row in rows) / len(rows), 4) if rows else 0.0,
        "group_means": {
            key: round(sum(values) / len(values), 4)
            for key, values in sorted(grouped.items())
        },
        "group_critical_pass_rates": {
            key: round(sum(values) / len(values), 4)
            for key, values in sorted(grouped_critical.items())
        },
        "cases": rows,
        "note": "Character n-gram F1 and declared invariants are regression signals, not standalone proof of translation quality.",
    }


def compare_reports(
    corpus: dict,
    baseline_results: dict[str, str],
    candidate_results: dict[str, str],
    *,
    baseline_corpus_fingerprint: str = "",
    candidate_corpus_fingerprint: str = "",
    baseline_source_identity: str = "",
    candidate_source_identity: str = "",
) -> dict:
    baseline = evaluate(
        corpus,
        baseline_results,
        result_corpus_fingerprint=baseline_corpus_fingerprint,
        source_identity=baseline_source_identity,
    )
    candidate = evaluate(
        corpus,
        candidate_results,
        result_corpus_fingerprint=candidate_corpus_fingerprint,
        source_identity=candidate_source_identity,
    )
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
    critical_group_keys = sorted(
        set(baseline["group_critical_pass_rates"])
        | set(candidate["group_critical_pass_rates"])
    )
    group_critical_pass_rate_deltas = {
        key: round(
            candidate["group_critical_pass_rates"].get(key, 0.0)
            - baseline["group_critical_pass_rates"].get(key, 0.0),
            4,
        )
        for key in critical_group_keys
    }
    return {
        "schema": "translateit.translation_quality.comparison.v1",
        "complete_result_sets": (
            baseline["complete_result_set"] and candidate["complete_result_set"]
        ),
        "corpus_fingerprint": corpus_fingerprint(corpus),
        "baseline_source_identity": baseline["source_identity"],
        "candidate_source_identity": candidate["source_identity"],
        "baseline_provenance_matches_corpus": baseline["provenance_matches_corpus"],
        "candidate_provenance_matches_corpus": candidate["provenance_matches_corpus"],
        "baseline_critical_failures": baseline["critical_failures"],
        "candidate_critical_failures": candidate["critical_failures"],
        "critical_regressions": critical_regressions,
        "critical_recoveries": critical_recoveries,
        "mean_char_ngram_f1_delta": round(
            candidate["mean_char_ngram_f1"] - baseline["mean_char_ngram_f1"],
            4,
        ),
        "group_mean_deltas": group_mean_deltas,
        "group_critical_pass_rate_deltas": group_critical_pass_rate_deltas,
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
        request = {
            "command": "translate",
            "request_kind": "standalone_text",
            "text": case["source"],
            "source_language": source_language,
            "target_language": target_language,
        }
        context_pairs = case.get("context_pairs")
        if context_pairs:
            request.update(
                {
                    "request_kind": "meeting_context_quality",
                    "context_pairs": context_pairs,
                    "meeting_lane": "you",
                    "meeting_session_id": "translation-quality-eval",
                    "meeting_generation": 1,
                }
            )
        requests.append({"case_id": case["id"], "request": request})
    return {
        "schema": "translateit.translation_quality.requests.v1",
        "corpus_fingerprint": corpus_fingerprint(corpus),
        "requests": requests,
    }


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
        baseline = load_result_bundle(args.baseline)
        candidate = load_result_bundle(args.candidate)
        report = compare_reports(
            corpus,
            baseline["results"],
            candidate["results"],
            baseline_corpus_fingerprint=baseline["corpus_fingerprint"],
            candidate_corpus_fingerprint=candidate["corpus_fingerprint"],
            baseline_source_identity=baseline["source_identity"],
            candidate_source_identity=candidate["source_identity"],
        )
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0 if report["promotion_safe_on_declared_critical_invariants"] else 1

    if args.results is None:
        parser.error("--results is required for evaluate")
    result_bundle = load_result_bundle(args.results)
    report = evaluate(
        corpus,
        result_bundle["results"],
        result_corpus_fingerprint=result_bundle["corpus_fingerprint"],
        source_identity=result_bundle["source_identity"],
    )
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["complete_result_set"] and report["critical_failures"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
