from __future__ import annotations

import argparse
import hashlib
import json
import re
import unicodedata
from collections import defaultdict
from pathlib import Path

SCHEMA = "translateit.tts_quality.v1"
ALLOWED_ARTIFACTS = {
    "clipping",
    "dropout",
    "repetition",
    "truncation",
    "unexpected_silence",
    "noise_burst",
    "unstable_pitch",
}


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


def word_error_rate(reference: str, hypothesis: str) -> float:
    ref = normalize(reference).split()
    hyp = normalize(hypothesis).split()
    if not ref:
        return 0.0 if not hyp else 1.0
    return edit_distance(ref, hyp) / len(ref)


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
    seen = set()
    for case in cases:
        case_id = str(case.get("id", "")).strip()
        if not case_id or case_id in seen:
            raise ValueError(f"invalid or duplicate case id: {case_id!r}")
        seen.add(case_id)
        if not str(case.get("category", "")).strip():
            raise ValueError(f"{case_id}: category required")
        if not str(case.get("text", "")).strip():
            raise ValueError(f"{case_id}: text required")
        for field in ("preserve", "required_any"):
            if field not in case or not isinstance(case[field], list):
                raise ValueError(f"{case_id}: {field} list required")
    return data


def validate_corpus(corpus: dict) -> dict:
    failures = []
    for case in corpus["cases"]:
        norm = normalize(case["text"])
        missing_literals = [
            item for item in case["preserve"] if normalize(item) not in norm
        ]
        missing_concepts = [
            group
            for group in case["required_any"]
            if not any(normalize(option) in norm for option in group)
        ]
        if missing_literals or missing_concepts:
            failures.append(
                {
                    "case_id": case["id"],
                    "reason": "reference text violates declared intelligibility invariants",
                }
            )
    return {
        "ok": not failures,
        "case_count": len(corpus["cases"]),
        "categories": sorted({case["category"] for case in corpus["cases"]}),
        "failures": failures,
    }


def load_result_bundle(path: Path) -> dict:
    raw = json.loads(path.read_text(encoding="utf-8"))
    rows = raw.get("results") if isinstance(raw, dict) else raw
    if not isinstance(rows, list):
        raise ValueError("results must be a list or an object containing results")
    output = {}
    for row in rows:
        case_id = str(row.get("case_id", "")).strip()
        if not case_id or case_id in output:
            raise ValueError(f"invalid or duplicate result case_id: {case_id!r}")
        similarity = row.get("speaker_similarity")
        if not isinstance(similarity, (int, float)) or not 0.0 <= float(similarity) <= 1.0:
            raise ValueError(f"{case_id}: speaker_similarity must be between 0 and 1")
        artifacts = row.get("artifact_flags", [])
        if (
            not isinstance(artifacts, list)
            or len(set(artifacts)) != len(artifacts)
            or any(item not in ALLOWED_ARTIFACTS for item in artifacts)
        ):
            raise ValueError(f"{case_id}: invalid artifact_flags")
        wav_sha256 = str(row.get("wav_sha256", "")).strip().lower()
        if not re.fullmatch(r"[0-9a-f]{64}", wav_sha256):
            raise ValueError(f"{case_id}: valid wav_sha256 required")
        output[case_id] = {
            "intelligibility_text": str(row.get("intelligibility_text", "")).strip(),
            "speaker_similarity": float(similarity),
            "artifact_flags": artifacts,
            "wav_sha256": wav_sha256,
        }
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


def evaluate(
    corpus: dict,
    results: dict,
    *,
    result_corpus_fingerprint: str = "",
    source_identity: str = "",
) -> dict:
    expected = {case["id"] for case in corpus["cases"]}
    missing = sorted(expected - results.keys())
    unknown = sorted(results.keys() - expected)
    expected_fingerprint = corpus_fingerprint(corpus)
    provenance_matches = (
        not result_corpus_fingerprint
        or result_corpus_fingerprint == expected_fingerprint
    )
    rows = []
    group_similarity = defaultdict(list)
    group_wer = defaultdict(list)
    group_critical = defaultdict(list)
    critical_failures = 0

    for case in corpus["cases"]:
        result = results.get(case["id"], {})
        transcript = str(result.get("intelligibility_text", ""))
        norm = normalize(transcript)
        missing_literals = [
            item for item in case["preserve"] if normalize(item) not in norm
        ]
        missing_concepts = [
            group
            for group in case["required_any"]
            if not any(normalize(option) in norm for option in group)
        ]
        artifacts = list(result.get("artifact_flags", []))
        critical_pass = not missing_literals and not missing_concepts and not artifacts
        if not critical_pass:
            critical_failures += 1
        similarity = float(result.get("speaker_similarity", 0.0))
        wer = word_error_rate(case["text"], transcript)
        category = case["category"]
        group_similarity[category].append(similarity)
        group_wer[category].append(wer)
        group_critical[category].append(critical_pass)
        rows.append(
            {
                "case_id": case["id"],
                "category": category,
                "speaker_similarity": round(similarity, 6),
                "intelligibility_wer": round(wer, 4),
                "artifact_flags": artifacts,
                "missing_literals": missing_literals,
                "missing_concepts": missing_concepts,
                "critical_pass": critical_pass,
                "wav_sha256": result.get("wav_sha256"),
            }
        )

    count = len(rows)
    return {
        "schema": "translateit.tts_quality.report.v1",
        "corpus_fingerprint": expected_fingerprint,
        "result_corpus_fingerprint": result_corpus_fingerprint or None,
        "source_identity": source_identity or None,
        "provenance_matches_corpus": provenance_matches,
        "complete_result_set": not missing and not unknown and provenance_matches,
        "missing_case_ids": missing,
        "unknown_case_ids": unknown,
        "critical_failures": critical_failures,
        "critical_pass_rate": round((count - critical_failures) / count, 4) if count else 0.0,
        "mean_speaker_similarity": round(
            sum(row["speaker_similarity"] for row in rows) / count, 6
        )
        if count
        else 0.0,
        "mean_intelligibility_wer": round(
            sum(row["intelligibility_wer"] for row in rows) / count, 4
        )
        if count
        else 0.0,
        "group_speaker_similarity": {
            key: round(sum(values) / len(values), 6)
            for key, values in sorted(group_similarity.items())
        },
        "group_intelligibility_wer": {
            key: round(sum(values) / len(values), 4)
            for key, values in sorted(group_wer.items())
        },
        "group_critical_pass_rates": {
            key: round(sum(values) / len(values), 4)
            for key, values in sorted(group_critical.items())
        },
        "cases": rows,
        "note": (
            "Speaker similarity, intelligibility transcript error and artifact flags are "
            "regression evidence only for the exact captured audio/runtime identity."
        ),
    }


def compare_reports(
    corpus: dict,
    baseline_results: dict,
    candidate_results: dict,
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
    artifact_regressions = []
    for case_id in sorted(baseline_cases):
        base_row = baseline_cases[case_id]
        cand_row = candidate_cases[case_id]
        if base_row["critical_pass"] and not cand_row["critical_pass"]:
            critical_regressions.append(case_id)
        elif not base_row["critical_pass"] and cand_row["critical_pass"]:
            critical_recoveries.append(case_id)
        if not base_row["artifact_flags"] and cand_row["artifact_flags"]:
            artifact_regressions.append(case_id)

    provenance_complete = (
        bool(baseline["source_identity"])
        and bool(candidate["source_identity"])
        and baseline["provenance_matches_corpus"]
        and candidate["provenance_matches_corpus"]
    )
    return {
        "schema": "translateit.tts_quality.comparison.v1",
        "corpus_fingerprint": corpus_fingerprint(corpus),
        "baseline_source_identity": baseline["source_identity"],
        "candidate_source_identity": candidate["source_identity"],
        "complete_result_sets": (
            baseline["complete_result_set"] and candidate["complete_result_set"]
        ),
        "critical_regressions": critical_regressions,
        "critical_recoveries": critical_recoveries,
        "artifact_regressions": artifact_regressions,
        "mean_speaker_similarity_delta": round(
            candidate["mean_speaker_similarity"] - baseline["mean_speaker_similarity"],
            6,
        ),
        "mean_intelligibility_wer_delta": round(
            candidate["mean_intelligibility_wer"] - baseline["mean_intelligibility_wer"],
            4,
        ),
        "promotion_provenance_complete": provenance_complete,
        "promotion_safe_on_declared_critical_invariants": (
            baseline["complete_result_set"]
            and candidate["complete_result_set"]
            and provenance_complete
            and not critical_regressions
            and not artifact_regressions
        ),
        "note": (
            "Promotion-safe means no new declared critical/artifact regression on matched "
            "evidence. It is not a standalone listening-quality verdict."
        ),
    }


def fixture_plan(corpus: dict) -> dict:
    return {
        "schema": "translateit.tts_quality.fixture_plan.v1",
        "corpus_fingerprint": corpus_fingerprint(corpus),
        "rules": [
            "Synthesize every line with the same selected voice/runtime settings within one comparison.",
            "Keep generated WAVs unchanged and record SHA-256 for every sample.",
            "Capture an intelligibility transcript independently from the synthesis text.",
            "Record only observed artifact flags; an empty list means none observed by that evidence process.",
            "Record exact source/runtime identity for baseline and candidate.",
        ],
        "fixtures": [
            {
                "case_id": case["id"],
                "category": case["category"],
                "text": case["text"],
                "suggested_filename": f"{case['id']}.wav",
            }
            for case in corpus["cases"]
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "command",
        choices=("validate-corpus", "fixture-plan", "evaluate", "compare"),
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
    if args.command == "fixture-plan":
        print(json.dumps(fixture_plan(corpus), ensure_ascii=False, indent=2))
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
    bundle = load_result_bundle(args.results)
    report = evaluate(
        corpus,
        bundle["results"],
        result_corpus_fingerprint=bundle["corpus_fingerprint"],
        source_identity=bundle["source_identity"],
    )
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["complete_result_set"] and report["critical_failures"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
