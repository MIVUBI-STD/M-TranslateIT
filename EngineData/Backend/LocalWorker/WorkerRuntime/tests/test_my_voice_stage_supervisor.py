"""Authored contract examples only; no GPU/device validation performed."""
import os
from unittest.mock import Mock, patch

import pytest
from my_voice_stage_supervisor import OwnedStageTree, StageGuardError, gpu_free_mib
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


def test_posix_root_exit_does_not_prove_child_process_group_empty():
    # Source contract; do not spawn or signal real processes in this fixture.
    from pathlib import Path
    source = Path(__file__).resolve().parents[1].joinpath(
        "my_voice_stage_supervisor.py"
    ).read_text(encoding="utf-8")
    assert "os.killpg(self.process.pid, 0)" in source
    assert "os.killpg(self.process.pid, signal.SIGKILL)" in source
    assert "start_new_session=os.name != \"nt\"" in source
    assert "self.confirm_posix_group_cleanup()" in source


@pytest.mark.skipif(os.name == "nt", reason="POSIX stage process groups are not Windows Jobs")
def test_posix_stage_cleanup_confirms_descendants_after_root_wait():
    # Mocked ownership only: never starts a process or sends a real signal.
    tree = object.__new__(OwnedStageTree)
    tree.handle = None
    root = Mock()
    root.returncode = 1
    tree.process = root
    with (
        patch.object(tree, "remaining", side_effect=[1, 0]) as remaining,
        patch.object(tree, "terminate") as terminate,
    ):
        tree.__exit__(None, None, None)
    terminate.assert_called_once_with()
    root.wait.assert_called_once_with(timeout=8)
    assert remaining.call_count == 2


@pytest.mark.skipif(os.name == "nt", reason="POSIX stage process groups are not Windows Jobs")
def test_posix_stage_cleanup_refuses_unconfirmed_descendants():
    tree = object.__new__(OwnedStageTree)
    tree.handle = None
    root = Mock()
    root.returncode = 1
    tree.process = root
    with (
        patch.object(tree, "remaining", side_effect=[1, 1, 1]) as remaining,
        patch.object(tree, "terminate") as terminate,
        patch("my_voice_stage_supervisor.time.monotonic", side_effect=[0.0, 0.0, 9.0]),
        patch("my_voice_stage_supervisor.time.sleep") as sleep,
        pytest.raises(StageGuardError, match="posix_group_cleanup_not_confirmed"),
    ):
        tree.__exit__(None, None, None)
    terminate.assert_called_once_with()
    root.wait.assert_called_once_with(timeout=8)
    sleep.assert_called_once_with(0.05)
    assert remaining.call_count == 3
