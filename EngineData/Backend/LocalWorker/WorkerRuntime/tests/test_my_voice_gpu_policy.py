from my_voice_gpu_policy import MIB, assess_memory, check_stage_budget
import pytest


def test_3070_8gb_can_be_admitted_with_free_headroom() -> None:
    status = assess_memory(8192 * MIB, 7 * 1024 * MIB)
    assert status["state"] == "admitted"
    assert status["reserve_mib"] >= 1536
    assert 3072 <= status["safe_budget_mib"] <= 5734


def test_external_pressure_blocks_without_guessing() -> None:
    result = assess_memory(8192 * MIB, 3 * 1024 * MIB)
    assert result["state"] == "blocked_gpu_headroom"
    assert result["safe_budget_mib"] < 3072


def test_invalid_telemetry_is_unknown() -> None:
    assert assess_memory(0, 0)["state"] == "unknown"
    assert assess_memory(4096, 8192)["state"] == "unknown"
    assert assess_memory(True, 10)["state"] == "unknown"


def test_recheck_never_expands_original_admission() -> None:
    report = {"state": "cuda_detected", "admission_state": "admitted", "safe_budget_mib": 4500}
    assert check_stage_budget(report, 4000) == 4000
    with pytest.raises(RuntimeError):
        check_stage_budget({"state": "cuda_detected", "admission_state": "blocked_gpu_headroom"}, 4000)
