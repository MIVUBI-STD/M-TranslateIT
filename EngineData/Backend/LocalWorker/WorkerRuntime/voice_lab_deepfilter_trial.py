"""Developer-only DeepFilterNet3 A/B trial for frozen My Voice recordings.

Never called by Meeting, STT, GPT-SoVITS training or voice approval.
No auto download, fallback engine, model promotion, or training WAV mutation.
Expected asset SHA256s must have been reviewed independently: supplying an
expected hash yourself only checks bytes, it is not provenance approval.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import subprocess
import sys
import wave
from pathlib import Path
from typing import Any

from voice_lab_source_pair import MAX_NATIVE_BYTES, _read_pcm16, inspect_frozen_source_pair

EXECUTABLE_NAME = "deep-filter-0.5.6-x86_64-pc-windows-msvc.exe"
MODEL_NAME = "DeepFilterNet3_onnx.tar.gz"
ATTENUATION_DB = 18
MAX_TAKES = 8


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def verify_asset(path: Path, name: str, expected_sha256: str, size_limit: int) -> str:
    if not re.fullmatch(r"[0-9a-fA-F]{64}", expected_sha256):
        raise ValueError("invalid_expected_sha256")
    if path.name != name or path.is_symlink() or not path.is_file():
        raise ValueError("asset_identity_invalid")
    if not 0 < path.stat().st_size <= size_limit:
        raise ValueError("asset_size_invalid")
    digest = sha256_file(path)
    if digest != expected_sha256.lower():
        raise ValueError("asset_checksum_mismatch")
    return digest


def read_manifest(path: Path, max_bytes: int) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise ValueError("metadata_missing")
    if not 0 < path.stat().st_size <= max_bytes:
        raise ValueError("metadata_size_invalid")
    try:
        value = json.loads(path.read_text("utf-8"))
    except (ValueError, OSError) as exc:
        raise ValueError("metadata_invalid") from exc
    if not isinstance(value, dict):
        raise ValueError("metadata_invalid")
    return value


def frozen_sources(dataset: Path, line_ids: list[int]) -> list[dict[str, Any]]:
    """Do not execute the denoiser on unverified or stale source pairs."""
    if not 1 <= len(line_ids) <= MAX_TAKES or len(line_ids) != len(set(line_ids)):
        raise ValueError("trial_selection_invalid")
    manifest = read_manifest(dataset / "dataset.json", 64 * 1024)
    report = read_manifest(dataset / "audio_quality.json", 256 * 1024)
    if (
        manifest.get("schema_version") != 1
        or manifest.get("authorized_voice_confirmed") is not True
    ):
        raise ValueError("dataset_unauthorized")
    if report.get("schema") != "voice_lab.audio_quality.v1":
        raise ValueError("quality_report_schema_invalid")
    takes = manifest.get("takes")
    evidence = report.get("takes")
    if not isinstance(takes, list) or not isinstance(evidence, list):
        raise ValueError("metadata_shape_invalid")
    accepted: dict[int, str] = {}
    for item in takes:
        if not isinstance(item, dict) or type(item.get("line_id")) is not int:
            raise ValueError("dataset_take_invalid")
        line_id, name = item["line_id"], item.get("wav_file")
        if line_id <= 0 or name != f"take_{line_id:04}.wav" or line_id in accepted:
            raise ValueError("dataset_take_invalid")
        accepted[line_id] = name
    quality_by_name: dict[str, Any] = {}
    for row in evidence:
        if not isinstance(row, dict) or not isinstance(row.get("wav_file"), str):
            raise ValueError("quality_report_invalid")
        if row["wav_file"] in quality_by_name:
            raise ValueError("quality_report_duplicate")
        quality_by_name[row["wav_file"]] = row
    if set(quality_by_name) != set(accepted.values()):
        raise ValueError("quality_report_dataset_mismatch")

    selected = []
    for line_id in line_ids:
        name = accepted.get(line_id)
        if name is None:
            raise ValueError("take_not_in_frozen_dataset")
        canonical = dataset / name
        native = dataset / "NativeRate" / name
        current = inspect_frozen_source_pair(canonical, native)
        recorded = quality_by_name[name].get("source_pair")
        if (
            not isinstance(recorded, dict)
            or not current.get("eligible_for_offline_enhancement")
            or current.get("status") != "signal_consistent"
            or recorded.get("status") != "signal_consistent"
            or current.get("canonical_sha256") != recorded.get("canonical_sha256")
            or current.get("native_sha256") != recorded.get("native_sha256")
        ):
            raise ValueError(f"source_pair_not_verified:{line_id}")
        selected.append({
            "line_id": line_id, "name": name, "native": native, "canonical": canonical,
            "native_sha256": current["native_sha256"],
            "canonical_sha256": current["canonical_sha256"],
        })
    return selected


def save_pcm16_wav(destination: Path, samples: Any, sample_rate: int) -> None:
    import numpy as np

    pcm = np.asarray(samples, dtype=np.float64)
    if pcm.ndim != 1 or not 0 < pcm.size <= sample_rate * 60 or not np.isfinite(pcm).all():
        raise ValueError("trial_pcm_invalid")
    converted = np.rint(np.clip(pcm, -1, 1) * 32767).astype("<i2")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(destination), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(sample_rate)
        writer.writeframes(converted.tobytes())


def prepare_48k(native: Path, out_file: Path) -> dict[str, Any]:
    from scipy.signal import resample_poly

    samples, rate, digest = _read_pcm16(native, MAX_NATIVE_BYTES)
    if rate != 48_000:
        divisor = math.gcd(rate, 48_000)
        samples = resample_poly(samples, 48_000 // divisor, rate // divisor)
    save_pcm16_wav(out_file, samples, 48_000)
    return {"native_sha256": digest, "source_rate_hz": rate}


def deepfilter_command(binary: Path, model: Path, out_dir: Path, in_files: list[Path]) -> list[str]:
    """The official v0.5.6 CLI writes each output using the input basename."""
    if not 1 <= len(in_files) <= MAX_TAKES:
        raise ValueError("trial_inputs_invalid")
    if any(not re.fullmatch(r"take_[0-9]{4,}\.wav", item.name) for item in in_files):
        raise ValueError("trial_filename_invalid")
    return [
        str(binary), "--model", str(model), "--atten-lim-db", str(ATTENUATION_DB),
        "--out-dir", str(out_dir), *map(str, in_files),
    ]


def inspect_enhanced(raw_file: Path, output: Path, derived_32k: Path) -> dict[str, Any]:
    """Structural check only; never a substitute for human speaker-fidelity review."""
    import numpy as np
    from scipy.signal import resample_poly

    source, a_rate, _ = _read_pcm16(raw_file, MAX_NATIVE_BYTES)
    enhanced, b_rate, enhanced_hash = _read_pcm16(output, MAX_NATIVE_BYTES)
    if a_rate != 48_000 or b_rate != 48_000:
        raise ValueError("enhanced_format_invalid")
    delta_ms = abs(len(source) - len(enhanced)) * 1000 / 48_000
    rms = float(np.sqrt(np.mean(enhanced * enhanced)))
    clipping = float(np.mean(np.abs(enhanced) >= 0.98))
    if delta_ms > 60 or rms < 0.004 or clipping > 0.025:
        raise ValueError("enhanced_signal_unusable")
    save_pcm16_wav(derived_32k, resample_poly(enhanced, 2, 3), 32_000)
    return {
        "status": "requires_human_voice_review",
        "training_approved": False,
        "enhanced_sha256": enhanced_hash,
        "derived_32k_sha256": sha256_file(derived_32k),
        "duration_delta_ms": round(delta_ms, 3),
        "enhanced_rms_dbfs": round(20 * math.log10(rms), 3),
        "enhanced_clipping_fraction": round(clipping, 6),
    }


def run_trial(
    dataset: Path, trial_dir: Path, binary: Path, model: Path,
    binary_sha256: str, model_sha256: str, line_ids: list[int],
) -> dict[str, Any]:
    if os.name != "nt":
        raise ValueError("trial_windows_only")
    binary_hash = verify_asset(binary, EXECUTABLE_NAME, binary_sha256, 64 * 1024 * 1024)
    model_hash = verify_asset(model, MODEL_NAME, model_sha256, 24 * 1024 * 1024)
    frozen = dataset.resolve(strict=True)
    root = trial_dir.resolve(strict=False)
    if root == frozen or root.is_relative_to(frozen) or frozen.is_relative_to(root):
        raise ValueError("trial_location_overlaps_dataset")
    if trial_dir.exists() or trial_dir.is_symlink():
        raise ValueError("trial_location_exists")
    source_rows = frozen_sources(frozen, line_ids)

    inputs = root / "inputs_48k"
    outputs = root / "outputs_48k"
    candidates = root / "candidates_32k"
    for folder in (inputs, outputs, candidates):
        folder.mkdir(parents=True, exist_ok=False)
    staged = []
    for take in source_rows:
        record = prepare_48k(take["native"], inputs / take["name"])
        if record["native_sha256"] != take["native_sha256"]:
            raise ValueError("native_changed_while_preparing")
        staged.append({**take, **record})
    cmd = deepfilter_command(binary.resolve(strict=True), model.resolve(strict=True),
                             outputs, [inputs / take["name"] for take in staged])
    try:
        completed = subprocess.run(
            cmd, cwd=str(root), stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            timeout=300, check=False,
        )
    except subprocess.TimeoutExpired as exc:
        raise ValueError("denoiser_timeout") from exc
    if completed.returncode != 0:
        raise ValueError("denoiser_failed")

    rows = []
    for take in staged:
        current = inspect_frozen_source_pair(take["canonical"], take["native"])
        if (
            current.get("canonical_sha256") != take["canonical_sha256"]
            or current.get("native_sha256") != take["native_sha256"]
            or current.get("status") != "signal_consistent"
        ):
            raise ValueError("frozen_source_mutated_during_trial")
        result = inspect_enhanced(
            inputs / take["name"], outputs / take["name"], candidates / take["name"]
        )
        rows.append({
            "line_id": take["line_id"], "native_sha256": take["native_sha256"],
            "canonical_sha256": take["canonical_sha256"], **result,
        })

    result = {
        "schema": "translateit.voice_lab_deepfilter_trial.v1",
        "experiment_only": True,
        "training_approved": False,
        "upstream_version": "v0.5.6",
        "model": "DeepFilterNet3_onnx",
        "attenuation_limit_db": ATTENUATION_DB,
        "post_filter": False,
        "binary_sha256": binary_hash,
        "model_sha256": model_hash,
        "takes": rows,
    }
    (root / "trial.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description="Offline My Voice DeepFilterNet3 A/B experiment")
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--trial-dir", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--model-sha256", required=True)
    parser.add_argument("--line-ids", type=int, nargs="+", required=True)
    args = parser.parse_args()
    try:
        run_trial(args.dataset, args.trial_dir, args.binary, args.model,
                  args.binary_sha256, args.model_sha256, args.line_ids)
    except (OSError, ValueError):
        # Deliberately no speech, filenames, or user device paths in logs.
        print("voice_lab:offline_denoiser_trial_failed", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
