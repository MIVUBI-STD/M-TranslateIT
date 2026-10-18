"""Unexecuted local source examples for run-scoped cooperative stop contracts."""

import json
from pathlib import Path
from types import SimpleNamespace

from my_voice_cooperative import (
    ACK_ENV,
    REQUEST_ENV,
    STAGE_ENV,
    TOKEN_ENV,
    gpt_epoch_boundary,
    requested,
    sovits_epoch_boundary,
)


def configure(monkeypatch, tmp_path: Path, stage: str):
    token = "a" * 64
    request, ack = tmp_path / "request.json", tmp_path / "ack.json"
    monkeypatch.setenv(TOKEN_ENV, token)
    monkeypatch.setenv(REQUEST_ENV, str(request))
    monkeypatch.setenv(ACK_ENV, str(ack))
    monkeypatch.setenv(STAGE_ENV, stage)
    monkeypatch.setenv("TRANSLATEIT_MYVOICE_STAGE_NONCE", "b" * 32)
    monkeypatch.setenv("TRANSLATEIT_MYVOICE_RUN_GENERATION", "3")
    monkeypatch.setenv(
        "TRANSLATEIT_MYVOICE_EPOCH_PROGRESS_PATH",
        str(tmp_path / "epoch_progress.json"),
    )
    import os

    request.write_text(
        json.dumps(
            {
                "run_token": token,
                "stage": stage,
                "writer_pid": os.getpid(),
                "stage_nonce": "b" * 32,
            }
        )
    )
    return ack


def test_sovits_never_yields_before_checkpoint_epoch(tmp_path, monkeypatch):
    ack = configure(monkeypatch, tmp_path, "sovits")
    directory = tmp_path / "logs_s2_v2ProPlus"
    directory.mkdir()
    (directory / "G_1.pth").write_bytes(b"x" * 4097)
    (directory / "D_1.pth").write_bytes(b"y" * 4097)
    assert not sovits_epoch_boundary(1, 2, str(tmp_path), "v2ProPlus")
    assert not ack.exists()
    from my_voice_checkpoint_pair import publish_pair

    publish_pair(directory, 1, 2)
    assert sovits_epoch_boundary(2, 2, str(tmp_path), "v2ProPlus")
    assert json.loads(ack.read_text())["kind"] == "checkpoint_candidate_not_resume_verified"


def test_gpt_boundary_only_after_checkpoint_candidate(tmp_path, monkeypatch):
    ack = configure(monkeypatch, tmp_path, "gpt")
    ckpts = tmp_path / "ckpt"
    ckpts.mkdir()
    trainer = SimpleNamespace(current_epoch=4, should_stop=False)
    import pytest

    with pytest.raises(RuntimeError, match="gpt_current_epoch_checkpoint_missing"):
        gpt_epoch_boundary(trainer, 5, str(ckpts))
    assert not trainer.should_stop and not ack.exists()
    (ckpts / "epoch=4-step=100.ckpt").write_bytes(b"x" * 4097)
    gpt_epoch_boundary(trainer, 5, str(ckpts))
    assert trainer.should_stop
    assert json.loads(ack.read_text())["stage"] == "gpt"
    progress = json.loads((tmp_path / "epoch_progress.json").read_text())
    assert progress["stage"] == "gpt"
    assert progress["epoch"] == 5


def test_mismatched_run_token_rejected_not_silently_ignored(tmp_path, monkeypatch):
    import pytest

    configure(monkeypatch, tmp_path, "sovits")
    monkeypatch.setenv(TOKEN_ENV, "b" * 64)
    with pytest.raises(RuntimeError, match="cooperative_request_identity_mismatch"):
        requested("sovits")


def test_manual_cancel_is_run_and_generation_bound(tmp_path, monkeypatch):
    from my_voice_cooperative import GENERATION_ENV, MANUAL_STOP_ENV, manual_stop_requested

    token = "a" * 64
    file = tmp_path / "manual-stop.json"
    monkeypatch.setenv(TOKEN_ENV, token)
    monkeypatch.setenv(GENERATION_ENV, "7")
    monkeypatch.setenv(MANUAL_STOP_ENV, str(file))
    assert not manual_stop_requested()
    file.write_text(
        json.dumps(
            {
                "run_token": token,
                "run_generation": 7,
                "kind": "manual_cooperative_stop",
            }
        )
    )
    assert manual_stop_requested()
    monkeypatch.setenv(GENERATION_ENV, "8")
    import pytest

    with pytest.raises(RuntimeError, match="manual_stop_identity_mismatch"):
        manual_stop_requested()


def test_manual_stop_rejects_malformed_signal(tmp_path, monkeypatch):
    import pytest

    from my_voice_cooperative import GENERATION_ENV, MANUAL_STOP_ENV, manual_stop_requested

    signal = tmp_path / "stop.json"
    monkeypatch.setenv(TOKEN_ENV, "a" * 64)
    monkeypatch.setenv(GENERATION_ENV, "3")
    monkeypatch.setenv(MANUAL_STOP_ENV, str(signal))
    signal.write_text("not json", encoding="utf-8")
    with pytest.raises(RuntimeError, match="manual_stop_signal_unavailable"):
        manual_stop_requested()


def test_corrupt_cooperative_request_is_a_failure_not_no_request(tmp_path, monkeypatch):
    import pytest

    configure(monkeypatch, tmp_path, "sovits")
    (tmp_path / "request.json").write_text("{truncated")
    with pytest.raises(RuntimeError, match="cooperative_request_unreadable"):
        requested("sovits")


def test_no_request_exists_means_continue(tmp_path, monkeypatch):
    configure(monkeypatch, tmp_path, "sovits")
    (tmp_path / "request.json").unlink()
    assert requested("sovits") is False
