"""Static source-contract fixtures only; no GPU, training or validation run."""
from pathlib import Path

import pytest

from my_voice_single_gpu_adapter import _replace_one, single_gpu_source


def test_exact_occurrence_required() -> None:
    with pytest.raises(RuntimeError, match="single_gpu_source_drift"):
        _replace_one("hello hello", "hello", "bye", "duplicate")
    assert _replace_one("hello", "hello", "bye", "single") == "bye"


def test_tampered_upstream_never_executes(tmp_path: Path) -> None:
    source = tmp_path / "s2_train.py"
    source.write_text("print('changed')", encoding="utf-8")
    with pytest.raises(RuntimeError, match="single_gpu_source_revision_mismatch"):
        single_gpu_source(source, "GPT_SoVITS/s2_train.py")


def test_unknown_upstream_rejected(tmp_path: Path) -> None:
    with pytest.raises(RuntimeError, match="single_gpu_source_unsupported"):
        single_gpu_source(tmp_path / "other.py", "other.py")
