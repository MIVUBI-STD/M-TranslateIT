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
import stat
import tempfile
from pathlib import Path
from typing import Any

MAX_FULL_BYTES = 8 * 1024 * 1024 * 1024
MAX_RECEIPT_BYTES = 4096
MAX_PAIR_FILES = 8


def file_identity(path: Path) -> dict[str, Any]:
    """Hash each checkpoint once and require stable file identity across I/O.

    The worker is the sole checkpoint writer and files have unique step names,
    but this guard also catches unexpected external modification or a path
    swap while reading. The Rust archive independently verifies copied bytes.
    """
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or not 4096 < before.st_size <= MAX_FULL_BYTES:
        raise RuntimeError("my_voice:pair_file_untrusted")
    digest = hashlib.sha256()
    length = 0
    flags = os.O_RDONLY | getattr(os, "O_BINARY", 0) | getattr(os, "O_NOFOLLOW", 0)
    try:
        handle = os.open(path, flags)
        with os.fdopen(handle, "rb") as stream:
            opened = os.fstat(stream.fileno())
            if (opened.st_dev, opened.st_ino, opened.st_size) != (
                before.st_dev, before.st_ino, before.st_size
            ):
                raise RuntimeError("my_voice:pair_file_changed_during_read")
            while True:
                block = stream.read(1024 * 1024)
                if not block:
                    break
                length += len(block)
                if length > MAX_FULL_BYTES:
                    raise RuntimeError("my_voice:pair_file_too_large")
                digest.update(block)
            closed = os.fstat(stream.fileno())
    except OSError as exc:
        raise RuntimeError("my_voice:pair_file_unavailable") from exc
    after = path.lstat()
    def identity(meta: Any) -> tuple[int, int, int, int, int]:
        return (meta.st_dev, meta.st_ino, meta.st_size, meta.st_mtime_ns, meta.st_ctime_ns)
    if (
        not stat.S_ISREG(after.st_mode)
        or identity(before) != identity(after)
        or identity(before) != identity(opened)
        or identity(before) != identity(closed)
        or length != before.st_size
    ):
        raise RuntimeError("my_voice:pair_file_changed_during_read")
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
        # Use the already-computed G/D hash receipt; never hash both
        # multi-gigabyte checkpoints again simply to update UI progress.
        from my_voice_epoch_progress import publish_progress
        publish_progress("sovits", epoch, target.name)
    finally:
        # Crash/interrupted writes leave a .pending file for recovery to
        # detect. os.replace consumes it on successful publication.
        # Deliberately do not unlink an uncommitted pair receipt here.
        pass


def verified_pair_for_epoch(directory: Path, epoch: int) -> bool:
    """Hash only the *requested epoch* pair, never earlier full checkpoints."""
    if directory.is_symlink() or not directory.is_dir() or type(epoch) is not int or epoch < 1:
        return False
    try:
        entries = list(directory.glob("pair_*.json"))
        if not entries or len(entries) > MAX_PAIR_FILES:
            return False
        matching: list[tuple[int, dict[str, Any]]] = []
        for path in entries:
            if (path.is_symlink() or not path.is_file()
                or not 0 < path.stat().st_size <= MAX_RECEIPT_BYTES):
                return False
            match = re.fullmatch(r"pair_([0-9]+)\.json", path.name)
            if not match:
                return False
            value = json.loads(path.read_text(encoding="utf-8"))
            step = int(match.group(1))
            if (
                type(value) is not dict or value.get("format") != 1
                or type(value.get("global_step")) is not int
                or value["global_step"] != step
                or type(value.get("epoch")) is not int
            ):
                return False
            if value["epoch"] == epoch:
                matching.append((step, value))
        # More than one receipt for the same epoch is ambiguous, not success.
        if len(matching) != 1:
            return False
        step, receipt = matching[0]
        return receipt == pair_receipt(directory, step, epoch)
    except (OSError, ValueError, TypeError, RuntimeError):
        return False
