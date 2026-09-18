from __future__ import annotations

import importlib.util
import json
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EVALUATOR = ROOT / "aggregate_quality_readiness.py"


def load_evaluator():
    spec = importlib.util.spec_from_file_location("translateit_quality_readiness", EVALUATOR)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def comparison(schema: str, candidate: str, safe: bool = True) -> dict:
    return {
        "schema": schema,
        "candidate_source_identity": candidate,
        "promotion_provenance_complete": True,
        "complete_result_sets": True,
        "promotion_safe_on_declared_critical_invariants": safe,
    }


def main() -> int:
    evaluator = load_evaluator()
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        identity = "local-sha"
        schemas = {
            "translation": "translateit.translation_quality.comparison.v1",
            "asr": "translateit.asr_quality.comparison.v1",
            "tts": "translateit.tts_quality.comparison.v1",
        }
        domains = {}
        for domain, schema in schemas.items():
            filename = f"{domain}.json"
            (root / filename).write_text(
                json.dumps(comparison(schema, identity)),
                encoding="utf-8",
            )
            domains[domain] = {
                "report": filename,
                "expected_candidate_source_identity": identity,
            }

        manifest = {
            "schema": "translateit.quality_readiness.manifest.v1",
            "release_identity": identity,
            "domains": domains,
        }
        manifest_path = root / "manifest.json"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

        ready = evaluator.evaluate_manifest(manifest_path)
        assert ready["ready_on_declared_quality_evidence"] is True
        assert ready["blockers"] == []
        assert all(row["report_sha256"] for row in ready["domains"].values())

        tts_path = root / "tts.json"
        tts_path.write_text(
            json.dumps(
                comparison(
                    "translateit.tts_quality.comparison.v1",
                    identity,
                    safe=False,
                )
            ),
            encoding="utf-8",
        )
        failed = evaluator.evaluate_manifest(manifest_path)
        assert failed["ready_on_declared_quality_evidence"] is False
        assert "tts:critical_regression" in failed["blockers"]

        tts_path.write_text(
            json.dumps(
                comparison(
                    "translateit.tts_quality.comparison.v1",
                    "different-actor",
                )
            ),
            encoding="utf-8",
        )
        mismatched = evaluator.evaluate_manifest(manifest_path)
        assert mismatched["ready_on_declared_quality_evidence"] is False
        assert "tts:candidate_identity_mismatch" in mismatched["blockers"]

    print(json.dumps({"ok": True, "domains": sorted(schemas)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
