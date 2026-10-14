"""CheckpointIO for one pinned, local GPT Lightning training run.

A full optimizer checkpoint is first written by Lightning's own
TorchCheckpointIO into the destination directory and then atomically
replaced. We never delete previously saved full checkpoints on this path.
A partially written .pending file is intentionally preserved as
attention evidence; it is never considered an optimizer-resumable artifact.
"""
from __future__ import annotations

import os
import tempfile
from pathlib import Path
from typing import Any

MAX_FULL_GPT_CHECKPOINT_BYTES = 8 * 1024 * 1024 * 1024


def validate_gpt_full_destination(path: str | Path) -> Path:
    target = Path(path)
    if (
        not target.is_absolute()
        or target.suffix != ".ckpt"
        or not target.parent.is_dir()
        or target.parent.is_symlink()
        or target.is_symlink()
    ):
        raise RuntimeError("my_voice:gpt_checkpoint_path_untrusted")
    return target


def create_atomic_lightning_checkpoint_io() -> Any:
    # Heavy Lightning import occurs only inside the dedicated GPT stage child.
    from pytorch_lightning.plugins.io import TorchCheckpointIO

    class AtomicLocalGPTCheckpointIO(TorchCheckpointIO):
        def save_checkpoint(
            self, checkpoint: dict[str, Any], path: str | Path,
            storage_options: Any = None,
        ) -> None:
            if storage_options is not None:
                raise RuntimeError("my_voice:gpt_remote_checkpoint_unsupported")
            target = validate_gpt_full_destination(path)
            fd, staged_name = tempfile.mkstemp(
                prefix=".pending-gpt-full-",
                suffix=".ckpt",
                dir=str(target.parent),
            )
            os.close(fd)
            staged = Path(staged_name)
            # Do not delete on exception: a crash may leave the same file.
            # Recovery rejects unfinished .pending files before cleanup.
            super().save_checkpoint(checkpoint, str(staged), storage_options)
            size = staged.stat().st_size
            if not 4096 < size <= MAX_FULL_GPT_CHECKPOINT_BYTES:
                raise RuntimeError("my_voice:gpt_full_checkpoint_size_invalid")
            with staged.open("rb") as stream:
                os.fsync(stream.fileno())
            # Atomic publication; for a Lightning "last.ckpt" replacement,
            # the old complete file stays visible until the new file is ready.
            os.replace(staged, target)

        def remove_checkpoint(self, path: str | Path) -> None:
            # This release uses save_top_k=-1. No optimizer checkpoint
            # deletion or pruning is authorized inside training.
            raise RuntimeError("my_voice:gpt_checkpoint_pruning_blocked")

    return AtomicLocalGPTCheckpointIO()
