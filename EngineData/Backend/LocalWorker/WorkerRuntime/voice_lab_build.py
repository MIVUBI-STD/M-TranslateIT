"""One-shot VoiceLab build child owned by the Rust desktop runtime.

This process is intentionally not a server and not a second daily AI worker. It
validates the frozen VoiceLab dataset, delegates the pinned GPT-SoVITS V2ProPlus
stages, and produces one reviewable candidate package.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import sys
import tempfile
from pathlib import Path
from typing import Any

from voice_lab_audio_quality import inspect_take_frames
from voice_lab_gpt_sovits import (
    ENGINE,
    ENGINE_REVISION,
    VoiceLabProviderError,
    wav_signal_metrics_pcm16,
)
from voice_lab_gpt_sovits_build import build_candidate
from voice_lab_source_pair import inspect_frozen_source_pair

SCHEMA_VERSION = 1
SILENCE_ABS_PCM16 = 128
MAX_SILENCE_FRACTION = 0.90
CLIPPING_ABS_PCM16 = 32_760
MAX_CLIPPING_FRACTION = 0.05
MIN_ACTIVE_RMS_PCM16 = 256.0
MAX_DC_OFFSET_ABS_PCM16 = 2_048.0
MAX_DATASET_ACTIVE_RMS_SPREAD_DB = 18.0


class BuildError(RuntimeError):
    pass


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise BuildError(f"invalid_json:{path.name}:{type(exc).__name__}") from exc
    if not isinstance(value, dict):
        raise BuildError(f"invalid_json_object:{path.name}")
    return value


def atomic_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    handle, temp_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=str(path.parent)
    )
    try:
        with os.fdopen(handle, "w", encoding="utf-8", newline="\n") as stream:
            json.dump(value, stream, ensure_ascii=False, indent=2)
            stream.write("\n")
        os.replace(temp_name, path)
    except Exception:
        try:
            os.unlink(temp_name)
        except OSError:
            pass
        raise


def write_status(
    path: Path, phase: str, message: str,
    run_token: str, run_generation: int, sequence: int,
) -> None:
    atomic_json(
        path,
        {
            "schema_version": SCHEMA_VERSION,
            "engine": ENGINE,
            "engine_revision": ENGINE_REVISION,
            "phase": phase,
            "message": message[:500],
            "run_token": run_token,
            "run_generation": run_generation,
            "sequence": sequence,
        },
    )


def validate_manifest(dataset_dir: Path) -> dict[str, Any]:
    manifest = read_json(dataset_dir / "dataset.json")
    if manifest.get("schema_version") != SCHEMA_VERSION:
        raise BuildError("dataset_schema_mismatch")
    if manifest.get("authorized_voice_confirmed") is not True:
        raise BuildError("authorization_required")
    takes = manifest.get("takes")
    held_out = manifest.get("held_out_lines")
    if not isinstance(takes, list) or len(takes) < 2:
        raise BuildError("insufficient_training_takes")
    if not isinstance(held_out, list) or not held_out:
        raise BuildError("held_out_lines_missing")
    return manifest


def validate_take_signal(path: Path) -> dict[str, float]:
    try:
        metrics = wav_signal_metrics_pcm16(path)
    except VoiceLabProviderError as exc:
        raise BuildError(str(exc)) from exc

    # Guided recording already rejects the same gross per-take failures at
    # review time. Re-check frozen files here in case accepted recordings were
    # later changed or corrupted before training.
    if metrics["silence_fraction"] >= MAX_SILENCE_FRACTION:
        raise BuildError(f"take_excessive_silence:{path.name}")
    if metrics["clipping_fraction"] >= MAX_CLIPPING_FRACTION:
        raise BuildError(f"take_severe_clipping:{path.name}")
    if metrics["active_rms"] < MIN_ACTIVE_RMS_PCM16:
        raise BuildError(f"take_signal_too_low:{path.name}")
    if metrics["dc_offset"] >= MAX_DC_OFFSET_ABS_PCM16:
        raise BuildError(f"take_dc_offset_too_high:{path.name}")
    return metrics


def validate_dataset_signal(dataset_dir: Path, manifest: dict[str, Any]) -> None:
    takes = manifest.get("takes")
    if not isinstance(takes, list):
        raise BuildError("insufficient_training_takes")
    metrics: list[dict[str, float]] = []
    reports: list[dict[str, Any]] = []
    for item in takes:
        if not isinstance(item, dict):
            raise BuildError("invalid_training_take")
        wav_file = str(item.get("wav_file", "")).strip()
        if not wav_file or Path(wav_file).name != wav_file:
            raise BuildError("invalid_training_take")
        path = dataset_dir / wav_file
        metrics.append(validate_take_signal(path))
        try:
            frames = inspect_take_frames(path)
        except ValueError as exc:
            raise BuildError(f"take_frame_analysis_failed:{wav_file}:{exc}") from exc
        reports.append({
            "wav_file": wav_file,
            **frames,
            "source_pair": inspect_frozen_source_pair(
                path, dataset_dir / "NativeRate" / wav_file
            ),
        })

    active_rms_values = [item["active_rms"] for item in metrics]
    if len(active_rms_values) >= 2:
        quietest = min(active_rms_values)
        loudest = max(active_rms_values)
        spread_db = 20.0 * math.log10(loudest / quietest)
        if spread_db > MAX_DATASET_ACTIVE_RMS_SPREAD_DB:
            raise BuildError("dataset_recording_level_inconsistent")

    # Advisory report lives inside the existing disposable, frozen build
    # dataset. No accepted take bytes, public manifest, or TTS engine change.
    atomic_json(
        dataset_dir / "audio_quality.json",
        {
            "schema": "voice_lab.audio_quality.v1",
            "method": "non_destructive_frame_energy",
            "provides_perceptual_quality_score": False,
            "takes": reports,
        },
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", required=True)
    parser.add_argument("--asr-model-root", required=True)
    parser.add_argument("--dataset-dir", required=True)
    parser.add_argument("--candidate-dir", required=True)
    parser.add_argument("--evaluation-dir", required=True)
    parser.add_argument("--work-dir", required=True)
    parser.add_argument("--status-path", required=True)
    parser.add_argument("--gpu-budget-mib", required=True, type=int)
    parser.add_argument("--run-token", required=True)
    parser.add_argument("--run-generation", required=True, type=int)
    args = parser.parse_args()

    status_path = Path(args.status_path).resolve()
    if not re.fullmatch(r"[0-9a-f]{64}", args.run_token) or args.run_generation <= 0:
        print("my_voice:run_identity_invalid", file=sys.stderr)
        return 2
    sequence = 0

    def status(phase: str, message: str) -> None:
        nonlocal sequence
        sequence += 1
        write_status(status_path, phase, message, args.run_token, args.run_generation, sequence)

    try:
        from my_voice_gpu_policy import MIN_STAGE_BUDGET_MIB
        if args.gpu_budget_mib < MIN_STAGE_BUDGET_MIB or args.gpu_budget_mib > 1024 * 1024:
            raise BuildError("gpu_budget_invalid")
        # Available to short-lived stage children. No implicit CPU fallback.
        os.environ["TRANSLATEIT_MYVOICE_GPU_BUDGET_MIB"] = str(args.gpu_budget_mib)
        dataset_dir = Path(args.dataset_dir).resolve()
        manifest = validate_manifest(dataset_dir)
        validate_dataset_signal(dataset_dir, manifest)
        status("preparing", "Preparing VoiceLab training data.")
        build_candidate(
            source_root=Path(args.source_root).resolve(),
            asr_model_root=Path(args.asr_model_root).resolve(),
            dataset_dir=dataset_dir,
            candidate_dir=Path(args.candidate_dir).resolve(),
            evaluation_dir=Path(args.evaluation_dir).resolve(),
            work_dir=Path(args.work_dir).resolve(),
            manifest=manifest,
            status_writer=status,
        )
        status("ready_for_review", "Voice Actor samples are ready for review.")
        return 0
    except (BuildError, VoiceLabProviderError) as exc:
        status("failed", str(exc))
        print(f"voice_lab_build_failed:{exc}", file=sys.stderr)
        return 2
    except Exception as exc:
        status("failed", f"unexpected:{type(exc).__name__}")
        print(f"voice_lab_build_failed:unexpected:{type(exc).__name__}", file=sys.stderr)
        return 3


if __name__ == "__main__":
    raise SystemExit(main())
