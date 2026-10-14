"""Authored filesystem evidence tests; not run or GPU-validated."""
import json
from pathlib import Path
import pytest
from my_voice_checkpoint_pair import publish_pair, verified_pair_for_epoch

def checkpoints(tmp_path: Path, step: int):
    (tmp_path / f"G_{step}.pth").write_bytes(b"G" * 8192)
    (tmp_path / f"D_{step}.pth").write_bytes(b"D" * 8192)

def test_matching_pair_manifest_published(tmp_path: Path):
    checkpoints(tmp_path, 120)
    publish_pair(tmp_path, 120, 2)
    assert verified_pair_for_epoch(tmp_path, 2)
    receipt = json.loads((tmp_path / "pair_120.json").read_text())
    assert receipt["global_step"] == 120
    assert receipt["generator"]["sha256"] != receipt["discriminator"]["sha256"]

def test_changed_generator_invalidates_pair(tmp_path: Path):
    checkpoints(tmp_path, 120)
    publish_pair(tmp_path, 120, 2)
    (tmp_path / "G_120.pth").write_bytes(b"Z" * 8192)
    assert not verified_pair_for_epoch(tmp_path, 2)

def test_missing_discriminator_never_publishes_pair(tmp_path: Path):
    (tmp_path / "G_120.pth").write_bytes(b"G" * 8192)
    with pytest.raises(RuntimeError):
        publish_pair(tmp_path, 120, 2)
    assert not (tmp_path / "pair_120.json").exists()
