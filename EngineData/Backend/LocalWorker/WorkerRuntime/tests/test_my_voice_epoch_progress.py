"""Unexecuted filesystem fixtures: no CUDA or training invoked."""
import json
import os
import pytest
from pathlib import Path
from my_voice_epoch_progress import publish_progress, read_progress
from my_voice_cooperative import TOKEN_ENV, GENERATION_ENV, STAGE_ENV

def setup(monkeypatch, tmp_path: Path):
    monkeypatch.setenv(TOKEN_ENV, "a" * 64)
    monkeypatch.setenv(GENERATION_ENV, "3")
    monkeypatch.setenv(STAGE_ENV, "sovits")
    monkeypatch.setenv("TRANSLATEIT_MYVOICE_STAGE_NONCE", "b" * 32)
    monkeypatch.setenv("TRANSLATEIT_MYVOICE_EPOCH_PROGRESS_PATH",
                       str(tmp_path / "epoch_progress.json"))

def test_monotonic_progress_is_run_scoped(tmp_path, monkeypatch):
    setup(monkeypatch, tmp_path)
    publish_progress("sovits", 2, "pair_100.json")
    assert read_progress(tmp_path, "a"*64, 3, "sovits", 0, os.getpid(), "b"*32) == 2
    assert read_progress(tmp_path, "a"*64, 3, "sovits", 2, os.getpid(), "b"*32) is None
    publish_progress("sovits", 4, "pair_200.json")
    assert read_progress(tmp_path, "a"*64, 3, "sovits", 2, os.getpid(), "b"*32) == 4
    with pytest.raises(RuntimeError, match="mismatch"):
        read_progress(tmp_path, "b"*64, 3, "sovits", 2, os.getpid(), "b"*32)

def test_rejected_when_progress_rolls_back(tmp_path, monkeypatch):
    setup(monkeypatch, tmp_path)
    publish_progress("sovits", 2, "pair_100.json")
    with pytest.raises(RuntimeError, match="rolled_back"):
        read_progress(tmp_path, "a"*64, 3, "sovits", 4, os.getpid(), "b"*32)


def test_nonprogress_interval_does_not_fake_new_epoch(tmp_path, monkeypatch):
    setup(monkeypatch, tmp_path)
    assert read_progress(tmp_path, "a"*64, 3, "sovits", 0, os.getpid(), "b"*32) is None
    publish_progress("sovits", 2, "pair_100.json")
    assert read_progress(tmp_path, "a"*64, 3, "sovits", 2, os.getpid(), "b"*32) is None


def test_completed_epoch_not_checkpoint(tmp_path, monkeypatch):
    from my_voice_epoch_progress import publish_epoch_activity, read_epoch_activity
    setup(monkeypatch, tmp_path)
    publish_epoch_activity("sovits", 1)
    assert read_epoch_activity(tmp_path, "a"*64, 3, "sovits", 0, os.getpid(), "b"*32) == 1
    assert read_progress(tmp_path, "a"*64, 3, "sovits", 0, os.getpid(), "b"*32) is None
    assert read_epoch_activity(tmp_path, "a"*64, 3, "sovits", 1) is None


def test_wrong_generation_never_counted_as_epoch(tmp_path, monkeypatch):
    from my_voice_epoch_progress import publish_epoch_activity, read_epoch_activity
    setup(monkeypatch, tmp_path)
    publish_epoch_activity("sovits", 1)
    with pytest.raises(RuntimeError, match="epoch_activity_identity_mismatch"):
        read_epoch_activity(tmp_path, "a"*64, 7, "sovits", 0, os.getpid(), "b"*32)


def test_stale_process_id_is_not_activity(tmp_path, monkeypatch):
    from my_voice_epoch_progress import publish_epoch_activity, read_epoch_activity
    setup(monkeypatch, tmp_path)
    publish_epoch_activity("sovits", 1)
    with pytest.raises(RuntimeError, match="epoch_activity_identity_mismatch"):
        read_epoch_activity(tmp_path, "a"*64, 3, "sovits", 0, os.getpid()+1, "b"*32)

def test_stage_nonce_is_not_interchangeable(tmp_path, monkeypatch):
    from my_voice_epoch_progress import publish_progress, read_progress
    setup(monkeypatch, tmp_path)
    publish_progress("sovits", 2, "pair_100.json")
    with pytest.raises(RuntimeError, match="checkpoint_progress_mismatch"):
        read_progress(tmp_path, "a"*64, 3, "sovits", 0, os.getpid(), "c"*32)
