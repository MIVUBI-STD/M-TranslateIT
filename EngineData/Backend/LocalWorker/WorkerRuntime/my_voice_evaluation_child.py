"""One-candidate, one-process My Voice held-out evaluator.

A short-lived GPU process isolates the TTS model from the other candidates.
Output is evidence, not an approved actor. Caller verifies all held-out WAV
SHA-256 values and exact identities before accepting this result.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any

from my_voice_gpu_policy import MIN_STAGE_BUDGET_MIB
from voice_lab_gpt_sovits_build import (
    create_evaluation_asr_runtime,
    evaluate_candidate,
    source_assets,
    stage_gpu_budget,
)

MAX_REQUEST_BYTES = 96 * 1024
MAX_RESULT_BYTES = 96 * 1024


def read_request(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > MAX_REQUEST_BYTES:
        raise RuntimeError("my_voice:evaluation_request_invalid")
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise RuntimeError("my_voice:evaluation_request_invalid")
    if not isinstance(payload.get("candidate"), dict) or not isinstance(payload.get("reference"), dict):
        raise RuntimeError("my_voice:evaluation_request_invalid")
    if not isinstance(payload.get("manifest"), dict):
        raise RuntimeError("my_voice:evaluation_request_invalid")
    return payload


def run_once(request: dict[str, Any]) -> dict[str, Any]:
    # This child runs on the same pinned Python dependency environment.
    import torch

    budget = stage_gpu_budget()
    total = torch.cuda.get_device_properties(0).total_memory // (1024 * 1024)
    if budget < MIN_STAGE_BUDGET_MIB or total < budget:
        raise RuntimeError("my_voice:evaluation_gpu_budget_invalid")
    torch.cuda.set_per_process_memory_fraction(min(0.70, budget / total), device=0)

    source_root = Path(str(request["source_root"])).resolve(strict=True)
    asr_root = Path(str(request["asr_model_root"])).resolve(strict=True)
    evaluation_root = Path(str(request["evaluation_root"])).resolve(strict=True)
    candidate = request["candidate"]
    manifest = request["manifest"]
    reference = request["reference"]
    held = manifest.get("held_out_lines")
    if not isinstance(held, list) or len(held) != 8:
        raise RuntimeError("my_voice:evaluation_held_out_invalid")
    candidate_id = str(candidate.get("candidate_id", ""))
    if not candidate_id or len(candidate_id) > 48 or not all(c.isalnum() or c == "-" for c in candidate_id):
        raise RuntimeError("my_voice:evaluation_candidate_invalid")
    paths = (str(candidate.get("gpt_path", "")), str(candidate.get("sovits_path", "")))
    if not all(Path(p).is_file() and not Path(p).is_symlink() for p in paths):
        raise RuntimeError("my_voice:evaluation_weights_missing")
    model = create_evaluation_asr_runtime(asr_root)
    evidence = evaluate_candidate(
        source_root, source_assets(source_root), candidate,
        evaluation_root, manifest, reference, model,
    )
    del model
    return evidence


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--request", required=True)
    parser.add_argument("--result", required=True)
    args = parser.parse_args()
    target = Path(args.result)
    if target.exists() or target.is_symlink():
        raise RuntimeError("my_voice:evaluation_result_exists")
    evidence = run_once(read_request(Path(args.request)))
    target.parent.mkdir(parents=True, exist_ok=True)
    encoded = json.dumps(evidence, default=str, ensure_ascii=True, separators=(",", ":")).encode("utf-8")
    if len(encoded) > MAX_RESULT_BYTES:
        raise RuntimeError("my_voice:evaluation_result_too_large")
    # Single-writer create-only report. Failure cannot yield a valid result file.
    with target.open("xb") as stream:
        stream.write(encoded)
        stream.flush()
        os.fsync(stream.fileno())
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"my_voice:evaluation_child_failed:{type(exc).__name__}", file=sys.stderr)
        raise SystemExit(3) from None
