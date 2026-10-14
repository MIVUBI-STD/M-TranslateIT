"""Unexecuted filesystem fixtures: no CUDA or training invoked."""
import json
import pytest
from pathlib import Path
from my_voice_epoch_progress import publish_progress, read_progress
from my_voice_cooperative import TOKEN_ENV, GENERATION_ENV, STAGE_ENV

def setup(monkeypatch, tmp_path: Path):
    monkeypatch.setenv(TOKEN_ENV, "a" * 64)
    monkeypatch.setenv(GENERATION_ENV, "3")
    monkeypatch.setenv(STAGE_ENV, "sovits")
    monkeypatch.setenv("TRANSLATEIT_MYVOICE_EPOCH_PROGRESS_PATH",
                       str(tmp_path / "epoch_progress.json"))

def test_monotonic_progress_is_run_scoped(tmp_path, monkeypatch):
    setup(monkeypatch, tmp_path)
    publish_progress("sovits", 2, "pair_100.json")
    assert read_progress(tmp_path, "a"*64, 3, "sovits", 0) == 2
    assert read_progress(tmp_path, "a"*64, 3, "sovits", 2) is None
    publish_progress("sovits", 4, "pair_200.json")
    assert read_progress(tmp_path, "a"*64, 3, "sovits", 2) == 4
    with pytest.raises(RuntimeError, match="mismatch"):
        read_progress(tmp_path, "b"*64, 3, "sovits", 2)

def test_rejected_when_progress_rolls_back(tmp_path, monkeypatch):
    setup(monkeypatch, tmp_path)
    publish_progress("sovits", 2, "pair_100.json")
    with pytest.raises(RuntimeError, match="rolled_back"):
        read_progress(tmp_path, "a"*64, 3, "sovits", 4)
