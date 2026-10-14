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



def test_sovits_inference_format_header_survives_atomic_publish(tmp_path):
    from my_voice_checkpoint_atomic import atomic_sovits_inference_save
    target = tmp_path / "translateit_myvoice_e2_s100.pth"

    def pinned_versioned_writer(payload, location, version):
        # Fixture models the pinned GPT-SoVITS two-byte version header.
        Path(location).write_bytes(b"06" + payload)

    atomic_sovits_inference_save(b"x" * 8192, str(target), "v2ProPlus", pinned_versioned_writer)
    assert target.read_bytes()[:2] == b"06"
    assert target.stat().st_size == 8194


def test_sovits_inference_writer_does_not_overwrite_previous_candidate(tmp_path):
    from my_voice_checkpoint_atomic import atomic_sovits_inference_save
    target = tmp_path / "translateit_myvoice_e2_s100.pth"
    target.write_bytes(b"previous")
    with pytest.raises(RuntimeError, match="checkpoint_unsafe"):
        atomic_sovits_inference_save(
            b"x" * 8192, str(target), "v2ProPlus",
            lambda payload, location, version: Path(location).write_bytes(payload),
        )
    assert target.read_bytes() == b"previous"
