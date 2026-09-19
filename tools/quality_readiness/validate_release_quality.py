from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

SCHEMA = "translateit.quality_readiness.report.v1"
DOMAINS = ("translation", "asr", "tts")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_release_quality(report_path: Path, release_identity: str) -> dict:
    data = json.loads(report_path.read_text(encoding="utf-8"))
    if not isinstance(data, dict) or data.get("schema") != SCHEMA:
        raise ValueError("quality_readiness_schema_invalid")
    expected_identity = release_identity.strip().lower()
    actual_identity = str(data.get("release_identity", "")).strip().lower()
    if not expected_identity or actual_identity != expected_identity:
        raise ValueError("quality_readiness_release_identity_mismatch")
    if data.get("ready_on_declared_quality_evidence") is not True:
        raise ValueError("quality_readiness_not_ready")
    if data.get("blockers") != []:
        raise ValueError("quality_readiness_blockers_present")
    domains = data.get("domains")
    if not isinstance(domains, dict) or set(domains) != set(DOMAINS):
        raise ValueError("quality_readiness_domains_invalid")
    for domain in DOMAINS:
        row = domains[domain]
        if not isinstance(row, dict) or row.get("ready") is not True:
            raise ValueError(f"quality_readiness_domain_not_ready:{domain}")
        report_hash = str(row.get("report_sha256", "")).strip().lower()
        if len(report_hash) != 64 or any(ch not in "0123456789abcdef" for ch in report_hash):
            raise ValueError(f"quality_readiness_domain_hash_invalid:{domain}")
    return {
        "schema": SCHEMA,
        "release_identity": actual_identity,
        "report_sha256": sha256_file(report_path),
        "domains": {
            domain: {
                "candidate_source_identity": domains[domain].get("candidate_source_identity"),
                "report_sha256": domains[domain]["report_sha256"],
            }
            for domain in DOMAINS
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--release-identity", required=True)
    args = parser.parse_args()
    try:
        result = validate_release_quality(args.report, args.release_identity)
    except (OSError, ValueError, json.JSONDecodeError) as exc:
        print(json.dumps({"ok": False, "blocker": str(exc)}, indent=2))
        return 1
    print(json.dumps({"ok": True, **result}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
