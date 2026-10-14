"""Authored source-level behavior cases, not GPU/Lightning acceptance."""
from pathlib import Path
import pytest
from my_voice_checkpoint_io import validate_gpt_full_destination

def test_disallow_relative_path(tmp_path: Path):
    with pytest.raises(RuntimeError, match="gpt_checkpoint_path_untrusted"):
        validate_gpt_full_destination("relative.ckpt")

def test_allow_existing_local_parent(tmp_path: Path):
    assert validate_gpt_full_destination(tmp_path / "epoch=4-step=100.ckpt").parent == tmp_path

def test_disallow_non_ckpt(tmp_path: Path):
    with pytest.raises(RuntimeError, match="gpt_checkpoint_path_untrusted"):
        validate_gpt_full_destination(tmp_path / "unexpected.json")
