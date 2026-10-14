"""Authored contract examples only; no GPU/device validation performed."""
from unittest.mock import patch
import pytest
from my_voice_stage_supervisor import StageGuardError, gpu_free_mib
from my_voice_gpu_identity import DeviceIdentityError

UUID = "GPU-11111111-2222-3333-4444-555555555555"

def test_sensor_failure_is_explicit():
    with patch("my_voice_stage_supervisor.sensor_memory_mib",
               side_effect=DeviceIdentityError("unreadable")):
        with pytest.raises(StageGuardError, match="unreadable"):
            gpu_free_mib("nvidia-smi", UUID)

def test_sensor_uses_the_selected_physical_uuid():
    with patch("my_voice_stage_supervisor.sensor_memory_mib",
               return_value=(8192, 5120)) as probe:
        assert gpu_free_mib("nvidia-smi", UUID) == 5120
        probe.assert_called_once_with(UUID, executable="nvidia-smi")


def test_windows_unassigned_root_cleanup_is_not_a_job_kill_only():
    # Source fixture; Windows Job Object assignment failure needs the exact
    # suspended Popen root terminated, not only its empty Job.
    from pathlib import Path
    source = Path(__file__).resolve().parents[1].joinpath(
        "my_voice_stage_supervisor.py"
    ).read_text(encoding="utf-8")
    assert "proc.kill()" in source
    assert "unowned_suspended_stage_cleanup_unconfirmed" in source
