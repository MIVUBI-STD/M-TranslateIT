"""Unexecuted source regression: exact run identity and sequence status."""

import json
from pathlib import Path

from voice_lab_build import write_status


def test_status_receipt_binds_to_run(tmp_path: Path):
    target = tmp_path / "status.json"
    write_status(target, "training", "phase", "a" * 64, 7, 2)
    data = json.loads(target.read_text(encoding="utf-8"))
    assert data["run_token"] == "a" * 64
    assert data["run_generation"] == 7
    assert data["sequence"] == 2
