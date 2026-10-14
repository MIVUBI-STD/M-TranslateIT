"""Create-only cooperative stop request and saved-epoch acknowledgment.

The acknowledgment means only that a trainer returned at a configured
checkpoint boundary. It does NOT prove optimizer resume, model validity,
or successful generation. The existing Rust recovery archive remains owner.
"""
from __future__ import annotations
import json
import os
from pathlib import Path

REQUEST_ENV = "TRANSLATEIT_MYVOICE_STOP_REQUEST"
ACK_ENV = "TRANSLATEIT_MYVOICE_STOP_ACK"
TOKEN_ENV = "TRANSLATEIT_MYVOICE_RUN_TOKEN"
STAGE_ENV = "TRANSLATEIT_MYVOICE_STOP_STAGE"

def requested(stage: str) -> bool:
    token = os.environ.get(TOKEN_ENV, "")
    if len(token) != 64 or any(c not in "0123456789abcdef" for c in token):
        return False
    name = os.environ.get(REQUEST_ENV, "")
    if not name or os.environ.get(STAGE_ENV) != stage:
        return False
    path = Path(name)
    try:
        if path.is_symlink() or not path.is_file() or path.stat().st_size > 4096:
            return False
        message = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return False
    return (isinstance(message, dict)
            and message.get("run_token") == token
            and message.get("stage") == stage)

def checkpoint_candidate_present(directory: Path, pattern: str) -> bool:
    if directory.is_symlink() or not directory.is_dir():
        return False
    try:
        parts = list(directory.glob(pattern))
        return bool(parts) and len(parts) <= 32 and all(
            file.is_file() and not file.is_symlink() and file.stat().st_size > 4096
            for file in parts
        )
    except OSError:
        return False

def write_ack(stage: str, epoch: int) -> None:
    target = os.environ.get(ACK_ENV, "")
    token = os.environ.get(TOKEN_ENV, "")
    if not target or not token:
        raise RuntimeError("my_voice:cooperative_ack_config_missing")
    body = json.dumps({
        "run_token": token, "stage": stage, "epoch": epoch,
        "kind": "checkpoint_candidate_not_resume_verified",
    }, sort_keys=True).encode("utf-8")
    with Path(target).open("xb") as stream:
        stream.write(body)
        stream.flush()
        os.fsync(stream.fileno())

def sovits_epoch_boundary(epoch: int, save_every: int, exp_dir: str, version: str) -> bool:
    if not requested("sovits") or save_every <= 0 or epoch % save_every != 0:
        return False
    from my_voice_checkpoint_pair import verified_pair_for_epoch
    directory = Path(exp_dir) / ("logs_s2_" + version)
    if not verified_pair_for_epoch(directory, epoch):
        return False
    write_ack("sovits", epoch)
    return True

def gpt_epoch_boundary(trainer: object, save_every: int, checkpoint_dir: str) -> None:
    epoch = int(getattr(trainer, "current_epoch", -1)) + 1
    if not requested("gpt") or save_every <= 0 or epoch % save_every != 0:
        return
    if not checkpoint_candidate_present(Path(checkpoint_dir), "*.ckpt"):
        return
    write_ack("gpt", epoch)
    # The existing GPT ModelCheckpoint hook runs first in callback order.
    # This sets a cooperative end-of-epoch stop, not a continuation claim.
    trainer.should_stop = True
