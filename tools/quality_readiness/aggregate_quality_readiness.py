from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

SCHEMA = "translateit.quality_readiness.manifest.v1"
REPORT_SCHEMAS = {
    "translation": "translateit.translation_quality.comparison.v1",
    "asr": "translateit.asr_quality.comparison.v1",
    "tts": "translateit.tts_quality.comparison.v1",
}


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

    return {
        "schema": "translateit.quality_readiness.report.v1",
        "release_identity": manifest["release_identity"],
        "ready_on_declared_quality_evidence": not blockers,
        "blockers": blockers,
        "domains": domain_rows,
        "boundary": (
            "This aggregates source/evaluation evidence only. It does not replace "
            "TARGET_WINDOWS microphone, meeting-route, latency, or listening acceptance."
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args()
    report = evaluate_manifest(args.manifest)
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["ready_on_declared_quality_evidence"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
