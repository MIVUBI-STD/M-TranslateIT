"""Authored negative and fake-writer fixtures; never run without consent."""
from pathlib import Path
import pytest
from my_voice_checkpoint_atomic import atomic_sovits_save

class FakeTorch:
    @staticmethod
    def save(payload, location):
        Path(location).write_bytes(payload)

def test_checkpoint_publishes_to_new_distinct_name(tmp_path):
    target = tmp_path / "G_20.pth"
    atomic_sovits_save(b"x" * 8192, str(target), FakeTorch)
    assert target.read_bytes() == b"x" * 8192

def test_failed_write_does_not_redefine_checkpoint(tmp_path):
    target = tmp_path / "D_21.pth"
    target.write_bytes(b"prior")
    with pytest.raises(RuntimeError, match="target_unsafe"):
        atomic_sovits_save(b"y" * 8192, str(target), FakeTorch)
    assert target.read_bytes() == b"prior"

def test_truncated_new_checkpoint_not_published(tmp_path):
    target = tmp_path / "G_22.pth"
    with pytest.raises(RuntimeError, match="size_invalid"):
        atomic_sovits_save(b"short", str(target), FakeTorch)
    assert not target.exists()
