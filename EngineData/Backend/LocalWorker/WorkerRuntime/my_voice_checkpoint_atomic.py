"""Same-directory atomic publication for *full* SoVITS optimizer checkpoints.

This only replaces the pinned upstream utils.my_save boundary during My Voice
training. Each checkpoint is written to a uniquely named pending file on the
same filesystem, synced and atomically published. SoVITS G/D pair is NOT one
atomic transaction; they remain recovery candidates until verified together.
"""
from __future__ import annotations

import os
import tempfile
from pathlib import Path
from typing import Any

MAX_CHECKPOINT_BYTES = 8 * 1024 * 1024 * 1024


def atomic_sovits_save(payload: Any, path: str, torch_module: Any) -> None:
    target = Path(path)
    if (
        target.name[:2] not in ("G_", "D_")
        or target.suffix != ".pth"
        or not target.name[2:-4].isdecimal()
        or not target.parent.is_dir()
        or target.parent.is_symlink()
        or target.exists() or target.is_symlink()
    ):
        raise RuntimeError("my_voice:optimizer_checkpoint_target_unsafe")
    fd, name = tempfile.mkstemp(
        prefix=".pending-" + target.stem + "-", suffix=".partial",
        dir=str(target.parent),
    )
    staged = Path(name)
    try:
        os.close(fd)
        torch_module.save(payload, str(staged))
        size = staged.stat().st_size
        if not 4096 < size <= MAX_CHECKPOINT_BYTES:
            raise RuntimeError("my_voice:optimizer_checkpoint_size_invalid")
        with staged.open("rb") as stream:
            os.fsync(stream.fileno())
        # No publication of a partially-written filename. Distinct global
        # step names avoid overwriting a previously complete checkpoint.
        if target.exists() or target.is_symlink():
            raise RuntimeError("my_voice:optimizer_checkpoint_collision")
        os.replace(staged, target)
    finally:
        # Preserve a failed stage's partial only as explicitly incomplete
        # diagnostic evidence; it is NEVER a valid G_/D_ checkpoint.
        if staged.exists():
            try:
                staged.unlink()
            except OSError:
                pass


def install_sovits_atomic_writer(utils_module: Any) -> None:
    import torch

    if getattr(utils_module, "_translateit_atomic_writer_installed", False):
        return

    def guarded_my_save(payload: Any, destination: str) -> None:
        atomic_sovits_save(payload, destination, torch)

    utils_module.my_save = guarded_my_save
    utils_module._translateit_atomic_writer_installed = True
