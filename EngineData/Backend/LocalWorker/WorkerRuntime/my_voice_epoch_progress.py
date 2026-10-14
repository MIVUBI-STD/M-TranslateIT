"""Run-scoped checkpoint milestone reporting; separate from supervisor heartbeat.

One overwriteable tiny JSON status is published atomically after the trainer
finishes an eligible checkpoint. It is NOT a successful run, loaded model or
optimizer resume receipt. Never hash full weights again just to signal progress.
"""
from __future__ import annotations

import json
import os
import re
import tempfile
from pathlib import Path
from typing import Any, Mapping

PROGRESS_ENV = "TRANSLATEIT_MYVOICE_EPOCH_PROGRESS_PATH"
TOKEN_ENV = "TRANSLATEIT_MYVOICE_RUN_TOKEN"
GENERATION_ENV = "TRANSLATEIT_MYVOICE_RUN_GENERATION"
STAGE_ENV = "TRANSLATEIT_MYVOICE_STOP_STAGE"
MAX_RECEIPT_BYTES = 1024
VALID_STAGES = {"sovits", "gpt"}


def _identity(env: Mapping[str, str], stage: str) -> tuple[str, int, Path]:
    token = env.get(TOKEN_ENV, "")
    number = env.get(GENERATION_ENV, "")
    path = env.get(PROGRESS_ENV, "")
    if (stage not in VALID_STAGES or env.get(STAGE_ENV) != stage
        or not re.fullmatch(r"[0-9a-f]{64}", token)
        or not number.isdecimal() or not 0 < int(number) < 2**64
        or not path or not Path(path).is_absolute()):
        raise RuntimeError("my_voice:checkpoint_progress_identity_missing")
    target = Path(path)
    if target.is_symlink() or not target.parent.is_dir() or target.parent.is_symlink():
        raise RuntimeError("my_voice:checkpoint_progress_path_untrusted")
    return token, int(number), target


def publish_progress(stage: str, epoch: int, checkpoint_filename: str) -> None:
    token, generation, target = _identity(os.environ, stage)
    if (type(epoch) is not int or not 1 <= epoch <= 10000
        or not checkpoint_filename or len(checkpoint_filename) > 100
        or not re.fullmatch(r"[A-Za-z0-9._=-]+", checkpoint_filename)
        or "/" in checkpoint_filename or "\\" in checkpoint_filename):
        raise RuntimeError("my_voice:checkpoint_progress_invalid")
    data = json.dumps({
        "format": 1, "run_token": token, "run_generation": generation,
        "stage": stage, "epoch": epoch, "checkpoint": checkpoint_filename,
        "kind": "checkpoint_written_not_resume_verified",
    }, sort_keys=True, separators=(",", ":")).encode("utf-8")
    if len(data) > MAX_RECEIPT_BYTES:
        raise RuntimeError("my_voice:checkpoint_progress_too_large")
    fd, temp = tempfile.mkstemp(prefix=".checkpoint-progress-", suffix=".tmp",
                                dir=str(target.parent))
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temp, target)
    finally:
        if os.path.exists(temp):
            os.unlink(temp)


def read_progress(
    directory: Path, token: str, generation: int, stage: str,
    previous_epoch: int,
) -> int | None:
    # Called at most once per existing stage supervisor monitoring cycle.
    path = directory / "epoch_progress.json"
    if not path.exists():
        if path.is_symlink():
            raise RuntimeError("my_voice:checkpoint_progress_symlink")
        return None
    if path.is_symlink() or not path.is_file() or path.stat().st_size > MAX_RECEIPT_BYTES:
        raise RuntimeError("my_voice:checkpoint_progress_untrusted")
    try:
        item: Any = json.loads(path.read_text(encoding="utf-8"))
    except (ValueError, OSError) as exc:
        raise RuntimeError("my_voice:checkpoint_progress_unreadable") from exc
    if (
        type(item) is not dict or item.get("format") != 1
        or item.get("run_token") != token or item.get("run_generation") != generation
        or item.get("stage") != stage
        or item.get("kind") != "checkpoint_written_not_resume_verified"
        or type(item.get("epoch")) is not int or not 1 <= item["epoch"] <= 10000
        or not isinstance(item.get("checkpoint"), str)
        or not re.fullmatch(r"[A-Za-z0-9._=-]{1,100}", item["checkpoint"])
    ):
        raise RuntimeError("my_voice:checkpoint_progress_mismatch")
    if item["epoch"] < previous_epoch:
        raise RuntimeError("my_voice:checkpoint_progress_rolled_back")
    return item["epoch"] if item["epoch"] > previous_epoch else None
