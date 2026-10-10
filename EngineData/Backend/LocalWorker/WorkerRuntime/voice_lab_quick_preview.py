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
from pathlib import Path

from voice_lab_gpt_sovits import (
    VoiceLabProviderError,
    prepare_pretrained_voice_preview,
    synthesize_pretrained_voice_preview,
)

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
        if not target.is_file() or target.stat().st_size <= 44:
            raise VoiceLabProviderError("preview_audio_invalid")
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
    parser.add_argument("--reference-wav", required=True, type=Path)
    parser.add_argument("--reference-text", required=True)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        response = preview_once(
            args.source_root, args.reference_wav, args.output_dir, args.reference_text
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
