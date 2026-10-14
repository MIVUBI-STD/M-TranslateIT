"""Content-bound SoVITS Generator/Discriminator checkpoint pair receipts.

Pair receipts are published only after BOTH complete distinct optimizer files
exist at the same global step. They prove matching filenames and on-disk byte
hashes, not tensor loadability, AMP state or safe optimizer resume.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import tempfile
from pathlib import Path
from typing import Any

MAX_FULL_BYTES = 8 * 1024 * 1024 * 1024
MAX_RECEIPT_BYTES = 4096
MAX_PAIR_FILES = 8


def file_identity(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise RuntimeError("my_voice:pair_file_untrusted")
    length = 0
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while True:
            block = stream.read(1024 * 1024)
            if not block:
                break
            length += len(block)
            if length > MAX_FULL_BYTES:
                raise RuntimeError("my_voice:pair_file_too_large")
            digest.update(block)
    if length <= 4096:
        raise RuntimeError("my_voice:pair_file_incomplete")
    return {"name": path.name, "bytes": length, "sha256": digest.hexdigest()}


def pair_receipt(directory: Path, step: int, epoch: int) -> dict[str, Any]:
    if directory.is_symlink() or not directory.is_dir():
        raise RuntimeError("my_voice:pair_directory_untrusted")
    if type(step) is not int or step < 1 or type(epoch) is not int or epoch < 1:
        raise RuntimeError("my_voice:pair_step_invalid")
    return {
        "format": 1, "global_step": step, "epoch": epoch,
        "generator": file_identity(directory / f"G_{step}.pth"),
        "discriminator": file_identity(directory / f"D_{step}.pth"),
    }


def publish_pair(directory: Path, step: int, epoch: int) -> None:
    receipt = pair_receipt(directory, step, epoch)
    target = directory / f"pair_{step}.json"
    if target.exists() or target.is_symlink():
        raise RuntimeError("my_voice:pair_receipt_already_exists")
    payload = json.dumps(receipt, sort_keys=True, separators=(",", ":")).encode("utf-8")
    if len(payload) > MAX_RECEIPT_BYTES:
        raise RuntimeError("my_voice:pair_receipt_too_large")
    fd, name = tempfile.mkstemp(
        prefix=".pending-pair-", suffix=".partial", dir=str(directory),
    )
    staged = Path(name)
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        if target.exists() or target.is_symlink():
            raise RuntimeError("my_voice:pair_receipt_collision")
        os.replace(staged, target)
    finally:
        # Preserve any abnormal partial as attention, never as a valid pair.
        pass


def verified_pair_for_epoch(directory: Path, epoch: int) -> bool:
    if directory.is_symlink() or not directory.is_dir() or type(epoch) is not int or epoch < 1:
        return False
    try:
        entries = list(directory.glob("pair_*.json"))
        if not entries or len(entries) > MAX_PAIR_FILES:
            return False
        for path in entries:
            if path.is_symlink() or not path.is_file() or path.stat().st_size > MAX_RECEIPT_BYTES:
                return False
            match = re.fullmatch(r"pair_([0-9]+)\.json", path.name)
            if not match:
                return False
            receipt = json.loads(path.read_text(encoding="utf-8"))
            step = int(match.group(1))
            if (type(receipt) is not dict or receipt.get("format") != 1
                or receipt.get("global_step") != step):
                return False
            if receipt == pair_receipt(directory, step, receipt.get("epoch")):
                if receipt["epoch"] == epoch:
                    return True
    except (OSError, ValueError, TypeError, RuntimeError):
        return False
    return False
