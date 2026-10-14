"""Create-only cooperative stop request and saved-epoch acknowledgment.

The acknowledgment means only that a trainer returned at a configured
checkpoint boundary. It does NOT prove optimizer resume, model validity,
or successful generation. The existing Rust recovery archive remains owner.
"""
from __future__ import annotations
import json
import os
from pathlib import Path
from typing import Mapping

REQUEST_ENV = "TRANSLATEIT_MYVOICE_STOP_REQUEST"
ACK_ENV = "TRANSLATEIT_MYVOICE_STOP_ACK"
TOKEN_ENV = "TRANSLATEIT_MYVOICE_RUN_TOKEN"
STAGE_ENV = "TRANSLATEIT_MYVOICE_STOP_STAGE"
STAGE_NONCE_ENV = "TRANSLATEIT_MYVOICE_STAGE_NONCE"
MANUAL_STOP_ENV = "TRANSLATEIT_MYVOICE_MANUAL_STOP_PATH"
GENERATION_ENV = "TRANSLATEIT_MYVOICE_RUN_GENERATION"


def manual_stop_requested(environment: Mapping[str, str] | None = None) -> bool:
    """Check the existing Rust run-bound stop signal without extra workers."""
    env = os.environ if environment is None else environment
    name = env.get(MANUAL_STOP_ENV, "")
    if not name:
        return False
    token = env.get(TOKEN_ENV, "")
    generation = env.get(GENERATION_ENV, "")
    if len(token) != 64 or not generation.isdecimal() or int(generation) < 1:
        raise RuntimeError("my_voice:manual_stop_identity_missing")
    path = Path(name)
    try:
        if not path.exists() and not path.is_symlink():
            return False
        if path.is_symlink() or not path.is_file() or path.stat().st_size > 4096:
            raise RuntimeError("my_voice:manual_stop_signal_untrusted")
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise RuntimeError("my_voice:manual_stop_signal_unavailable") from exc
    if not isinstance(payload, dict) or (
        payload.get("run_token") != token
        or payload.get("run_generation") != int(generation)
        or payload.get("kind") != "manual_cooperative_stop"
    ):
        raise RuntimeError("my_voice:manual_stop_identity_mismatch")
    return True

def requested(stage: str) -> bool:
    """Fail closed when an existing trainer stop request is unreadable/foreign."""
    token = os.environ.get(TOKEN_ENV, "")
    name = os.environ.get(REQUEST_ENV, "")
    if not name:
        return False
    if len(token) != 64 or any(c not in "0123456789abcdef" for c in token):
        raise RuntimeError("my_voice:cooperative_request_identity_missing")
    if os.environ.get(STAGE_ENV) != stage:
        raise RuntimeError("my_voice:cooperative_request_stage_mismatch")
    path = Path(name)
    try:
        if not path.exists() and not path.is_symlink():
            return False
        if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= 4096:
            raise RuntimeError("my_voice:cooperative_request_untrusted")
        message = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise RuntimeError("my_voice:cooperative_request_unreadable") from exc
    if (not isinstance(message, dict)
        or message.get("run_token") != token
        or message.get("stage") != stage
        or type(message.get("writer_pid")) is not int
        or message.get("writer_pid") != os.getpid()
        or not os.environ.get(STAGE_NONCE_ENV)
        or message.get("stage_nonce") != os.environ.get(STAGE_NONCE_ENV)):
        raise RuntimeError("my_voice:cooperative_request_identity_mismatch")
    return True


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
        "writer_pid": os.getpid(),
        "stage_nonce": os.environ.get(STAGE_NONCE_ENV, ""),
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
    if save_every <= 0 or epoch % save_every != 0:
        return
    # This function is called only *after* pinned ModelCheckpoint has
    # completed its full save and latest-save operations for this epoch.
    # A checkpoint from a previous epoch alone can never authorize yielding.
    candidates = [
        p for p in Path(checkpoint_dir).glob(f"epoch={epoch-1}-step=*.ckpt")
        if p.is_file() and not p.is_symlink() and p.stat().st_size > 4096
    ]
    if not candidates:
        raise RuntimeError("my_voice:gpt_current_epoch_checkpoint_missing")
    # A filesystem milestone, not a claim that checkpoint can resume the
    # optimizer. Only one stat/filename needed; never re-read full weights.
    from my_voice_epoch_progress import publish_progress
    publish_progress("gpt", epoch, sorted(candidates, key=lambda p: p.name)[0].name)
    if not requested("gpt"):
        return
    write_ack("gpt", epoch)
    # Mark the current trainer for graceful stop AFTER its save completed.
    # This does not assert optimizer-resumable training state.
    trainer.should_stop = True
