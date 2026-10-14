"""Authored filesystem evidence tests; not run or GPU-validated."""
import json
from pathlib import Path
import pytest
from my_voice_checkpoint_pair import publish_pair, verified_pair_for_epoch
from my_voice_cooperative import TOKEN_ENV, GENERATION_ENV, STAGE_ENV

@pytest.fixture(autouse=True)
def set_checkpoint_progress_identity(monkeypatch, tmp_path: Path):
    monkeypatch.setenv(TOKEN_ENV, "a" * 64)
    monkeypatch.setenv(GENERATION_ENV, "3")
    monkeypatch.setenv(STAGE_ENV, "sovits")
    monkeypatch.setenv("TRANSLATEIT_MYVOICE_STAGE_NONCE", "b" * 32)
    monkeypatch.setenv(
        "TRANSLATEIT_MYVOICE_EPOCH_PROGRESS_PATH",
        str(tmp_path / "epoch_progress.json"),
    )

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


def test_pair_lookup_hashes_only_the_target_epoch(tmp_path, monkeypatch):
    from unittest.mock import patch
    import my_voice_checkpoint_pair as pair
    checkpoints(tmp_path, 120)
    publish_pair(tmp_path, 120, 2)
    checkpoints(tmp_path, 250)
    publish_pair(tmp_path, 250, 4)
    with patch.object(pair, "pair_receipt", wraps=pair.pair_receipt) as receipt:
        assert verified_pair_for_epoch(tmp_path, 4)
        assert receipt.call_count == 1
        assert receipt.call_args.args[1:] == (250, 4)


def test_modified_checkpoint_rejected_without_loading_tensor(tmp_path):
    import os
    checkpoints(tmp_path, 120)
    publish_pair(tmp_path, 120, 2)
    (tmp_path / "D_120.pth").write_bytes(b"z" * 8192)
    assert not verified_pair_for_epoch(tmp_path, 2)


def test_pair_file_lstat_rejects_symlink_without_loading_payload(tmp_path):
    from my_voice_checkpoint_pair import file_identity
    original = tmp_path / "G_101.pth"
    original.write_bytes(b"G" * 8192)
    link = tmp_path / "G_102.pth"
    link.symlink_to(original)
    with pytest.raises(RuntimeError, match="pair_file_untrusted"):
        file_identity(link)
