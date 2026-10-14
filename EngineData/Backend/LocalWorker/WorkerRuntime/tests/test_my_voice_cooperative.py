"""Unexecuted local source examples for run-scoped cooperative stop contracts."""
import json
from pathlib import Path
from types import SimpleNamespace

from my_voice_cooperative import (
    requested, sovits_epoch_boundary, gpt_epoch_boundary,
    REQUEST_ENV, ACK_ENV, TOKEN_ENV, STAGE_ENV,
)

def configure(monkeypatch, tmp_path: Path, stage: str):
    token = "a" * 64
    request, ack = tmp_path / "request.json", tmp_path / "ack.json"
    monkeypatch.setenv(TOKEN_ENV, token)
    monkeypatch.setenv(REQUEST_ENV, str(request))
    monkeypatch.setenv(ACK_ENV, str(ack))
    monkeypatch.setenv(STAGE_ENV, stage)
    request.write_text(json.dumps({"run_token": token, "stage": stage}))
    return ack

def test_sovits_never_yields_before_checkpoint_epoch(tmp_path, monkeypatch):
    ack = configure(monkeypatch, tmp_path, "sovits")
    directory = tmp_path / "logs_s2_v2ProPlus"
    directory.mkdir()
    (directory / "G_1.pth").write_bytes(b"x" * 4097)
    (directory / "D_1.pth").write_bytes(b"y" * 4097)
    assert not sovits_epoch_boundary(1, 2, str(tmp_path), "v2ProPlus")
    assert not ack.exists()
    assert sovits_epoch_boundary(2, 2, str(tmp_path), "v2ProPlus")
    assert json.loads(ack.read_text())["kind"] == "checkpoint_candidate_not_resume_verified"

def test_gpt_boundary_only_after_checkpoint_candidate(tmp_path, monkeypatch):
    ack = configure(monkeypatch, tmp_path, "gpt")
    ckpts = tmp_path / "ckpt"
    ckpts.mkdir()
    trainer = SimpleNamespace(current_epoch=4, should_stop=False)
    gpt_epoch_boundary(trainer, 5, str(ckpts))
    assert not trainer.should_stop and not ack.exists()
    (ckpts / "epoch=4-step=100.ckpt").write_bytes(b"x" * 4097)
    gpt_epoch_boundary(trainer, 5, str(ckpts))
    assert trainer.should_stop
    assert json.loads(ack.read_text())["stage"] == "gpt"

def test_mismatched_run_token_ignored(tmp_path, monkeypatch):
    configure(monkeypatch, tmp_path, "sovits")
    monkeypatch.setenv(TOKEN_ENV, "b" * 64)
    assert not requested("sovits")
