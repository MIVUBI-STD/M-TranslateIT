from __future__ import annotations

import importlib.util
import json
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
VALIDATOR = ROOT / "validate_release_quality.py"


def load_validator():
    spec = importlib.util.spec_from_file_location("translateit_release_quality", VALIDATOR)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def ready_report(identity: str) -> dict:
    return {
        "schema": "translateit.quality_readiness.report.v1",
        "release_identity": identity,
        "ready_on_declared_quality_evidence": True,
        "blockers": [],
        "domains": {
            domain: {
                "ready": True,
                "candidate_source_identity": identity,
                "report_sha256": char * 64,
            }
            for domain, char in (
                ("translation", "a"),
                ("asr", "b"),
                ("tts", "c"),
            )
        },
    }


def main() -> int:
    validator = load_validator()
    identity = "1" * 40
    with tempfile.TemporaryDirectory() as raw:
        path = Path(raw) / "quality-readiness.json"
        path.write_text(json.dumps(ready_report(identity)), encoding="utf-8")
        result = validator.validate_release_quality(path, identity)
        assert result["release_identity"] == identity
        assert len(result["report_sha256"]) == 64

        mismatch = ready_report(identity)
        mismatch["release_identity"] = "2" * 40
        path.write_text(json.dumps(mismatch), encoding="utf-8")
        try:
            validator.validate_release_quality(path, identity)
        except ValueError as exc:
            assert str(exc) == "quality_readiness_release_identity_mismatch"
        else:
            raise AssertionError("identity mismatch must fail closed")

        failed = ready_report(identity)
        failed["domains"]["tts"]["ready"] = False
        path.write_text(json.dumps(failed), encoding="utf-8")
        try:
            validator.validate_release_quality(path, identity)
        except ValueError as exc:
            assert str(exc) == "quality_readiness_domain_not_ready:tts"
        else:
            raise AssertionError("domain readiness failure must fail closed")

    print(json.dumps({"ok": True}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
