from __future__ import annotations

import importlib.util
import copy
import hashlib
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
        translation_path = root / "translation.json"
        original_translation = translation_path.read_bytes()
        original_digest = hashlib.sha256(original_translation).hexdigest()
        assert ready["domains"]["translation"]["report_sha256"] == original_digest

        # Deterministic file mutation between report parsing and final output:
        # digest must identify the evaluated bytes, not the replaced file.
        original_loader = evaluator.load_report

        def mutate_after_load(path, schema):
            parsed, digest = original_loader(path, schema)
            if path == translation_path:
                path.write_text(json.dumps(comparison(
                    schema, "changed-after-read"
                )), encoding="utf-8")
            return parsed, digest

        evaluator.load_report = mutate_after_load
        try:
            pinned = evaluator.evaluate_manifest(manifest_path)
        finally:
            evaluator.load_report = original_loader
        assert pinned["domains"]["translation"]["candidate_source_identity"] == identity
        assert pinned["domains"]["translation"]["report_sha256"] == original_digest
        translation_path.write_bytes(original_translation)

        # A broken domain comparison is a structured blocker, not a crash.
        asr_path = root / "asr.json"
        old_asr = asr_path.read_bytes()
        asr_path.write_text("{malformed", encoding="utf-8")
        invalid = evaluator.evaluate_manifest(manifest_path)
        assert "asr:report_invalid_or_unreadable" in invalid["blockers"]
        assert invalid["domains"]["asr"]["report_sha256"] is None
        asr_path.write_bytes(old_asr)

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


        # Contract-only fixture: these hashes do not represent observed audio,
        # model output, or human evaluation. It tests metadata linkage only.
        case = {
            "case_id": "negation-001",
            "risk_tags": ["negation", "correction"],
            "source_wav_sha256": "a" * 64,
            "asr": {
                "audio_sha256": "a" * 64,
                "transcript_sha256": "b" * 64,
                "status": "complete",
            },
            "translation": {
                "input_transcript_sha256": "b" * 64,
                "output_text_sha256": "c" * 64,
                "direction": "id-en",
                "complete": True,
            },
            "tts": {
                "input_text_sha256": "c" * 64,
                "wav_sha256": "d" * 64,
                "status": "complete",
            },
            "delivery": {"wav_sha256": "d" * 64, "status": "output_complete"},
            "meaning_review": {"verdict": "no_critical_error", "reviewer_count": 1},
        }
        receipt = {
            "schema": evaluator.MEETING_TRACE_SCHEMA,
            "source_identity": identity,
            "domain_source_identities": {
                name: identity for name in schemas
            },
            "cases": [case],
        }
        manifest["meeting_trace"] = {
            "report": "meeting-trace.json",
            "expected_case_ids": ["negation-001"],
        }
        trace_path = root / "meeting-trace.json"
        trace_path.write_text(json.dumps(receipt), encoding="utf-8")
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        linked = evaluator.evaluate_manifest(manifest_path)
        assert linked["ready_on_declared_quality_evidence"] is False
        assert "tts:candidate_identity_mismatch" in linked["blockers"]
        # Restore domain comparison identity before proving optional trace wiring.
        tts_path.write_text(json.dumps(comparison(
            "translateit.tts_quality.comparison.v1", identity,
        )), encoding="utf-8")
        linked = evaluator.evaluate_manifest(manifest_path)
        assert linked["ready_on_declared_quality_evidence"] is True
        assert linked["meeting_trace"]["ready"] is True
        assert linked["meeting_trace"]["case_count"] == 1
        assert len(linked["meeting_trace"]["report_sha256"]) == 64

        # One behavior matrix, rather than one source-shape microtest per field.
        # These are synthetic metadata-only fixtures, not real quality proof.
        for label, case_patch, receipt_patch, expected_blocker in [
            ("tts mismatch", {"tts": {
                **case["tts"], "input_text_sha256": "e" * 64,
            }}, {}, "meeting_trace:negation-001:tts_boundary"),
            ("critical meaning", {"meaning_review": {
                "verdict": "critical_error", "reviewer_count": 1,
            }}, {}, "meeting_trace:negation-001:meaning_review_incomplete"),
            ("private raw text", {"asr": {
                **case["asr"], "transcript_text": "private text",
            }}, {}, "meeting_trace:negation-001:asr_boundary"),
            ("invalid stage", {"tts": []}, {},
             "meeting_trace:negation-001:invalid_stage_shape"),
            ("model mismatch", {}, {"domain_source_identities": {
                **receipt["domain_source_identities"], "asr": "different-model",
            }}, "meeting_trace:domain_identity_mismatch"),
            ("duplicate case", {}, {"cases": [case, case]},
             "meeting_trace:duplicate_case_id"),
        ]:
            altered = copy.deepcopy(receipt)
            altered["cases"][0].update(case_patch)
            altered.update(receipt_patch)
            trace_path.write_text(json.dumps(altered), encoding="utf-8")
            rejected = evaluator.evaluate_manifest(manifest_path)
            assert not rejected["ready_on_declared_quality_evidence"], label
            assert expected_blocker in rejected["blockers"], label

        trace_path.write_text(json.dumps(receipt), encoding="utf-8")
        manifest["meeting_trace"]["report"] = "../outside.json"
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        try:
            evaluator.load_manifest(manifest_path)
        except ValueError as exc:
            assert "local JSON filename" in str(exc)
        else:
            raise AssertionError("trace path traversal must fail closed")

        manifest_path.write_text(json.dumps([]), encoding="utf-8")
        try:
            evaluator.load_manifest(manifest_path)
        except ValueError as exc:
            assert str(exc) == "manifest must be a JSON object"
        else:
            raise AssertionError("non-object manifest must fail explicitly")

        # Legacy domain-only inputs remain accepted unchanged.
        manifest.pop("meeting_trace")
        manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        assert "meeting_trace" not in evaluator.evaluate_manifest(manifest_path)

    print(json.dumps({"ok": True, "domains": sorted(schemas)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
