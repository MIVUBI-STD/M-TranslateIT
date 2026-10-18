"""Source-level helper fixtures; evaluation and training are NOT executed."""
import json
from pathlib import Path

import pytest

from my_voice_evaluation_child import read_request

def test_request_rejects_untyped_candidate(tmp_path: Path):
    request = tmp_path / "r.json"
    request.write_text(json.dumps({"candidate": None, "reference": {}, "manifest": {}}))
    with pytest.raises(RuntimeError, match="evaluation_request_invalid"):
        read_request(request)

def test_request_rejects_symlink(tmp_path: Path):
    request = tmp_path / "r.json"
    request.symlink_to(tmp_path / "missing")
    with pytest.raises(RuntimeError, match="evaluation_request_invalid"):
        read_request(request)
