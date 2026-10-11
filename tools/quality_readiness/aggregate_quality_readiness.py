from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path

SCHEMA = "translateit.quality_readiness.manifest.v1"
REPORT_SCHEMAS = {
    "translation": "translateit.translation_quality.comparison.v1",
    "asr": "translateit.asr_quality.comparison.v1",
    "tts": "translateit.tts_quality.comparison.v1",
}


# Optional evidence for the same captured utterance across existing domain owners.
# This correlates declared hashes and review status; it never re-scores model output.
MEETING_TRACE_SCHEMA = "translateit.meeting_fidelity.trace.v1"
MAX_MEETING_TRACE_BYTES = 2 * 1024 * 1024
MAX_MEETING_CASES = 64
CASE_ID = re.compile(r"[a-z0-9][a-z0-9_-]{0,63}\\Z")
RISK_TAG = re.compile(r"[a-z][a-z0-9_-]{0,39}\\Z")
SHA256 = re.compile(r"[0-9a-f]{64}\\Z")


def keys_equal(value: object, required: set[str]) -> bool:
    return isinstance(value, dict) and set(value) == required


def valid_sha(value: object) -> bool:
    return isinstance(value, str) and SHA256.fullmatch(value) is not None


def local_json_name(value: object) -> bool:
    return (
        isinstance(value, str)
        and value.endswith(".json")
        and value != ".json"
        and Path(value).name == value
        and "/" not in value
        and chr(92) not in value
    )


def meeting_trace_blockers(
    receipt: dict, source_identity: str, expected_cases: list[str], domains: dict
) -> list[str]:
    if not keys_equal(
        receipt, {"schema", "source_identity", "domain_source_identities", "cases"}
    ) or receipt["schema"] != MEETING_TRACE_SCHEMA:
        return ["meeting_trace:invalid_schema"]
    blockers = []
    if receipt["source_identity"] != source_identity:
        blockers.append("meeting_trace:source_identity_mismatch")
    if receipt["domain_source_identities"] != domains:
        blockers.append("meeting_trace:domain_identity_mismatch")
    cases = receipt["cases"]
    if not isinstance(cases, list) or not 1 <= len(cases) <= MAX_MEETING_CASES:
        return blockers + ["meeting_trace:invalid_cases"]
    seen = set()
    for case in cases:
        if not keys_equal(case, {
            "case_id", "risk_tags", "source_wav_sha256", "asr",
            "translation", "tts", "delivery", "meaning_review"
        }):
            return blockers + ["meeting_trace:invalid_case_shape"]
        case_id = case["case_id"]
        if not isinstance(case_id, str) or CASE_ID.fullmatch(case_id) is None:
            return blockers + ["meeting_trace:invalid_case_id"]
        if case_id in seen:
            return blockers + ["meeting_trace:duplicate_case_id"]
        seen.add(case_id)
        tags = case["risk_tags"]
        if (not isinstance(tags, list) or not 1 <= len(tags) <= 8
            or any(not isinstance(tag, str) or RISK_TAG.fullmatch(tag) is None for tag in tags)
            or len(set(tags)) != len(tags)):
            blockers.append(f"meeting_trace:{case_id}:invalid_risk_tags")
        asr, translation, tts, delivery, review = (
            case["asr"], case["translation"], case["tts"],
            case["delivery"], case["meaning_review"]
        )
        if not all(isinstance(stage, dict) for stage in (
            asr, translation, tts, delivery, review
        )):
            blockers.append(f"meeting_trace:{case_id}:invalid_stage_shape")
            continue
        if not keys_equal(asr, {"audio_sha256", "transcript_sha256", "status"}) or (
            asr["status"] != "complete"
            or not valid_sha(case["source_wav_sha256"])
            or not valid_sha(asr["transcript_sha256"])
            or asr["audio_sha256"] != case["source_wav_sha256"]
        ):
            blockers.append(f"meeting_trace:{case_id}:asr_boundary")
        if not keys_equal(translation, {
            "input_transcript_sha256", "output_text_sha256", "direction", "complete"
        }) or (
            translation["direction"] != "id-en"
            or translation["complete"] is not True
            or not valid_sha(translation["output_text_sha256"])
            or translation["input_transcript_sha256"] != asr.get("transcript_sha256")
        ):
            blockers.append(f"meeting_trace:{case_id}:translation_boundary")
        if not keys_equal(tts, {"input_text_sha256", "wav_sha256", "status"}) or (
            tts["status"] != "complete"
            or not valid_sha(tts["wav_sha256"])
            or tts["input_text_sha256"] != translation.get("output_text_sha256")
        ):
            blockers.append(f"meeting_trace:{case_id}:tts_boundary")
        if not keys_equal(delivery, {"wav_sha256", "status"}) or (
            delivery["status"] != "output_complete"
            or delivery["wav_sha256"] != tts.get("wav_sha256")
        ):
            blockers.append(f"meeting_trace:{case_id}:delivery_boundary")
        if not keys_equal(review, {"verdict", "reviewer_count"}) or (
            review["verdict"] != "no_critical_error"
            or type(review["reviewer_count"]) is not int
            or not 1 <= review["reviewer_count"] <= 8
        ):
            blockers.append(f"meeting_trace:{case_id}:meaning_review_incomplete")
    if sorted(seen) != sorted(expected_cases):
        blockers.append("meeting_trace:case_set_mismatch")
    return blockers


def evaluate_meeting_trace(
    root: Path, config: dict, release_identity: str, candidate_domains: dict
) -> tuple[dict, list[str]]:
    path = root / config["report"]
    summary = {
        "ready": False, "report": config["report"], "report_sha256": None,
        "case_count": 0
    }
    if not path.is_file() or path.is_symlink():
        return summary, ["meeting_trace:report_missing_or_link"]
    if path.stat().st_size > MAX_MEETING_TRACE_BYTES:
        return summary, ["meeting_trace:report_too_large"]
    summary["report_sha256"] = sha256_file(path)
    try:
        receipt = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(receipt, dict):
            raise ValueError("receipt not an object")
    except (ValueError, UnicodeError):
        return summary, ["meeting_trace:report_invalid_json"]
    if isinstance(receipt.get("cases"), list):
        summary["case_count"] = len(receipt["cases"])
    blockers = meeting_trace_blockers(
        receipt, release_identity, config["expected_case_ids"], candidate_domains
    )
    summary["ready"] = not blockers
    return summary, blockers


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_manifest(path: Path) -> dict:
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get("schema") != SCHEMA:
        raise ValueError(f"unsupported manifest schema: {data.get('schema')!r}")
    release_identity = str(data.get("release_identity", "")).strip()
    if not release_identity:
        raise ValueError("release_identity required")
    domains = data.get("domains")
    if not isinstance(domains, dict) or set(domains) != set(REPORT_SCHEMAS):
        raise ValueError("manifest must declare exactly translation, asr, and tts domains")
    for domain, item in domains.items():
        if not isinstance(item, dict):
            raise ValueError(f"{domain}: domain declaration must be an object")
        report = str(item.get("report", "")).strip()
        expected_candidate = str(item.get("expected_candidate_source_identity", "")).strip()
        if not report or Path(report).name != report:
            raise ValueError(f"{domain}: report must be a local filename")
        if not expected_candidate:
            raise ValueError(f"{domain}: expected_candidate_source_identity required")
    if "meeting_trace" in data:
        trace = data["meeting_trace"]
        if not keys_equal(trace, {"report", "expected_case_ids"}):
            raise ValueError("meeting_trace: manifest shape invalid")
        if not local_json_name(trace["report"]):
            raise ValueError("meeting_trace: report must be a local JSON filename")
        expected = trace["expected_case_ids"]
        if (not isinstance(expected, list)
            or not 1 <= len(expected) <= MAX_MEETING_CASES
            or any(not isinstance(case_id, str) or CASE_ID.fullmatch(case_id) is None
                   for case_id in expected)
            or len(set(expected)) != len(expected)):
            raise ValueError("meeting_trace: invalid expected_case_ids")
    return data


def load_report(path: Path, expected_schema: str) -> dict:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError(f"{path.name}: comparison report must be an object")
    if data.get("schema") != expected_schema:
        raise ValueError(
            f"{path.name}: expected schema {expected_schema!r}, got {data.get('schema')!r}"
        )
    return data


def evaluate_manifest(manifest_path: Path) -> dict:
    manifest = load_manifest(manifest_path)
    root = manifest_path.parent
    domain_rows = {}
    blockers = []

    for domain, expected_schema in REPORT_SCHEMAS.items():
        config = manifest["domains"][domain]
        report_path = root / config["report"]
        if not report_path.is_file():
            blockers.append(f"{domain}:report_missing")
            domain_rows[domain] = {
                "ready": False,
                "report": config["report"],
                "report_sha256": None,
            }
            continue

        report = load_report(report_path, expected_schema)
        candidate_identity = str(report.get("candidate_source_identity") or "").strip()
        expected_candidate = config["expected_candidate_source_identity"]
        provenance_complete = report.get("promotion_provenance_complete") is True
        complete_sets = report.get("complete_result_sets") is True
        safe = report.get("promotion_safe_on_declared_critical_invariants") is True
        identity_matches = candidate_identity == expected_candidate

        if not provenance_complete:
            blockers.append(f"{domain}:provenance_incomplete")
        if not complete_sets:
            blockers.append(f"{domain}:result_sets_incomplete")
        if not identity_matches:
            blockers.append(f"{domain}:candidate_identity_mismatch")
        if not safe:
            blockers.append(f"{domain}:critical_regression")

        domain_rows[domain] = {
            "ready": provenance_complete and complete_sets and identity_matches and safe,
            "report": config["report"],
            "report_sha256": sha256_file(report_path),
            "candidate_source_identity": candidate_identity or None,
            "expected_candidate_source_identity": expected_candidate,
            "promotion_provenance_complete": provenance_complete,
            "complete_result_sets": complete_sets,
            "promotion_safe_on_declared_critical_invariants": safe,
        }

    meeting_trace = None
    if "meeting_trace" in manifest:
        candidate_domains = {
            domain: domain_rows[domain].get("candidate_source_identity")
            for domain in REPORT_SCHEMAS
        }
        meeting_trace, trace_blockers = evaluate_meeting_trace(
            root, manifest["meeting_trace"],
            manifest["release_identity"], candidate_domains,
        )
        blockers.extend(trace_blockers)

    output = {
        "schema": "translateit.quality_readiness.report.v1",
        "release_identity": manifest["release_identity"],
        "ready_on_declared_quality_evidence": not blockers,
        "blockers": blockers,
        "domains": domain_rows,
        "boundary": (
            "This aggregates source/evaluation evidence only. It does not replace "
            "TARGET_WINDOWS microphone, meeting-route, latency, or listening acceptance. "
            "Optional Meeting trace hashes and review declarations are not independent "
            "proof of model behavior, linguistic quality, or Meeting-app reception."
        ),
    }
    if meeting_trace is not None:
        output["meeting_trace"] = meeting_trace
    return output


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args()
    report = evaluate_manifest(args.manifest)
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["ready_on_declared_quality_evidence"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
