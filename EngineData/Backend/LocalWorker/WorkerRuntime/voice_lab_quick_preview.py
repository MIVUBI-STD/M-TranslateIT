"""Isolated GPT-SoVITS V2ProPlus zero-shot preview child.

No Meeting command, actor approval, model selector, or alternative voice engine.
The future Rust owner must exclusively lock the existing VoiceLab resource
before launching this child and must manage process cancellation.
"""
from __future__ import annotations

import argparse
import gc
import json
import os
import sys
import wave
from pathlib import Path

from voice_lab_gpt_sovits import (
    VoiceLabProviderError,
    prepare_pretrained_voice_preview,
    synthesize_pretrained_voice_preview,
)
from voice_lab_build import BuildError, validate_take_signal
from voice_lab_gpt_sovits_build import select_reference, synthesis_artifact_flags

PREVIEW_TEXT = "Good morning, everyone. Thank you for joining this meeting."
OUTPUT_FILE = "quick_voice_preview.wav"
MAX_REFERENCE_TEXT_CHARS = 512
MAX_STATUS_BYTES = 4096


def validate_preview_inputs(
    source_root: Path,
    reference_wav: Path,
    output_dir: Path,
    reference_text: str,
) -> Path:
    """Fail closed before model loading, with no write to approved actor."""
    if not source_root.is_dir():
        raise VoiceLabProviderError("preview_source_missing")
    if reference_wav.is_symlink() or not reference_wav.is_file():
        raise VoiceLabProviderError("preview_reference_missing")
    reference_text = reference_text.strip()
    if not reference_text or len(reference_text) > MAX_REFERENCE_TEXT_CHARS:
        raise VoiceLabProviderError("preview_reference_text_invalid")
    if not output_dir.is_dir() or output_dir.is_symlink():
        raise VoiceLabProviderError("preview_output_dir_invalid")
    if output_dir.resolve() == reference_wav.parent.resolve():
        raise VoiceLabProviderError("preview_output_must_be_separate")
    target = output_dir / OUTPUT_FILE
    if target.is_symlink() or target.exists():
        raise VoiceLabProviderError("preview_output_already_exists")
    return target


def validate_preview_wav(path: Path, expected_rate: int) -> None:
    """Verify the generated PCM WAV before exposing it to the UI."""
    try:
        with wave.open(str(path), "rb") as wav:
            frames = wav.getnframes()
            rate = wav.getframerate()
            if (wav.getnchannels() != 1 or wav.getsampwidth() != 2
                    or rate != expected_rate or not 16_000 <= rate <= 48_000
                    or not rate // 10 <= frames <= rate * 30):
                raise VoiceLabProviderError("preview_audio_invalid")
            audio = wav.readframes(frames)
            if len(audio) != frames * 2:
                raise VoiceLabProviderError("preview_audio_invalid")
        import numpy as np

        if synthesis_artifact_flags(np.frombuffer(audio, dtype="<i2"), rate):
            raise VoiceLabProviderError("preview_audio_artifacts")
    except VoiceLabProviderError:
        raise
    except (OSError, EOFError, ValueError, wave.Error) as exc:
        raise VoiceLabProviderError("preview_audio_invalid") from exc


def choose_reference(candidates: list[str]) -> tuple[Path, str]:
    """Reuse full-training signal admission and reference ranking."""
    # The product has 128 fixed guided lines; refuse unbounded child payloads.
    if not 1 <= len(candidates) <= 128:
        raise VoiceLabProviderError("preview_reference_candidate_invalid")
    takes = []
    seen_ids: set[int] = set()
    for serialized in candidates:
        try:
            item = json.loads(serialized)
            line_id = item["line_id"]
            exact_text = item["exact_text"]
            path_value = item["wav_path"]
            if (type(line_id) is not int or line_id <= 0 or line_id in seen_ids
                    or not isinstance(exact_text, str) or not exact_text.strip()
                    or len(exact_text) > MAX_REFERENCE_TEXT_CHARS
                    or not isinstance(path_value, str) or not path_value):
                raise ValueError("invalid candidate fields")
            path = Path(path_value)
            if path.is_symlink():
                raise ValueError("reference symlink not allowed")
        except (TypeError, ValueError, KeyError, OSError) as exc:
            raise VoiceLabProviderError("preview_reference_candidate_invalid") from exc
        seen_ids.add(line_id)
        # A once-accepted take can disappear or become unusable. Other accepted
        # takes should still be considered; unknown metadata remains fail-closed.
        if not path.is_file():
            continue
        try:
            metrics = validate_take_signal(path)
        except BuildError:
            continue
        takes.append({
            "line_id": line_id,
            "exact_text": exact_text.strip(),
            "wav_path": path,
            "duration_ms": int(metrics["duration_ms"]),
        })
    if not takes:
        raise VoiceLabProviderError("preview_reference_unusable")
    selected = select_reference(takes)
    return Path(selected["wav_path"]), str(selected["exact_text"])


def preview_once(
    source_root: Path,
    reference_wav: Path,
    output_dir: Path,
    reference_text: str,
) -> dict[str, object]:
    target = validate_preview_inputs(source_root, reference_wav, output_dir, reference_text)
    runtime = None
    try:
        runtime = prepare_pretrained_voice_preview(source_root, reference_wav, reference_text)
        result = synthesize_pretrained_voice_preview(runtime, PREVIEW_TEXT, target)
        validate_preview_wav(target, int(result["sample_rate"]))
        return {
            "ok": True,
            "stage": "quick_voice_preview",
            "preview_only": True,
            "file": OUTPUT_FILE,
            "sample_rate": int(result["sample_rate"]),
        }
    except Exception:
        target.unlink(missing_ok=True)
        raise
    finally:
        if runtime is not None:
            runtime.clear()
        gc.collect()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--reference-candidate", action="append", required=True)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        reference_wav, reference_text = choose_reference(args.reference_candidate)
        response = preview_once(
            args.source_root, reference_wav, args.output_dir, reference_text
        )
    except Exception as exc:
        response = {
            "ok": False,
            "stage": "quick_voice_preview",
            "preview_only": True,
            "blocker": str(exc) if isinstance(exc, VoiceLabProviderError) else "preview_unexpected_failure",
        }
    serialized = json.dumps(response, ensure_ascii=True)
    if len(serialized.encode("utf-8")) > MAX_STATUS_BYTES:
        raise RuntimeError("preview_status_too_large")
    sys.stdout.write(serialized + "\n")
    sys.stdout.flush()
    return 0 if response["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
