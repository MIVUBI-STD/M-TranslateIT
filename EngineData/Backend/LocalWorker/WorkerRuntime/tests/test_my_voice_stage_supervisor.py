"""Author-only regression fixtures. Do not run without user validation approval."""
from unittest.mock import patch
import subprocess
import pytest
from my_voice_stage_supervisor import StageGuardError, gpu_free_mib

def test_unreadable_sensor_fails_closed():
    with patch("my_voice_stage_supervisor.subprocess.run",
               return_value=subprocess.CompletedProcess([], 0, "invalid", "")):
        with pytest.raises(StageGuardError, match="gpu_sensor_unreadable"):
            gpu_free_mib("nvidia-smi")

def test_sensor_reads_free_mib():
    with patch("my_voice_stage_supervisor.subprocess.run",
               return_value=subprocess.CompletedProcess([], 0, "5120\n", "")):
        assert gpu_free_mib("nvidia-smi") == 5120
