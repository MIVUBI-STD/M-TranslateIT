"""Conservative, versioned CUDA-admission arithmetic for My Voice.

All thresholds are initial safety policy, not a benchmark or performance proof.
Checking headroom is not a reservation of GPU memory. No silent CPU fallback.
"""
from __future__ import annotations

from typing import Any

MIB = 1024 * 1024
PROFILE = "SAFE_8GB_V1"
MIN_RESERVE_MIB = 1536
MIN_STAGE_BUDGET_MIB = 3072
MAX_TOTAL_FRACTION_PERMILLE = 700
RESERVE_TOTAL_FRACTION_PERMILLE = 200


def assess_memory(total_bytes: int, free_bytes: int) -> dict[str, Any]:
    if (
        type(total_bytes) is not int or type(free_bytes) is not int
        or total_bytes <= 0 or free_bytes <= 0 or free_bytes > total_bytes
    ):
        return {"state": "unknown", "reserve_mib": None, "safe_budget_mib": None}
    total_mib = total_bytes // MIB
    free_mib = free_bytes // MIB
    reserve_mib = max(
        MIN_RESERVE_MIB,
        total_mib * RESERVE_TOTAL_FRACTION_PERMILLE // 1000,
    )
    safe_budget_mib = min(
        total_mib * MAX_TOTAL_FRACTION_PERMILLE // 1000,
        max(0, free_mib - reserve_mib),
    )
    return {
        "state": "admitted" if safe_budget_mib >= MIN_STAGE_BUDGET_MIB else "blocked_gpu_headroom",
        "reserve_mib": reserve_mib,
        "safe_budget_mib": safe_budget_mib,
    }


def check_stage_budget(status: dict[str, Any], originally_admitted_mib: int) -> int:
    """Fail closed if free memory changed; never increase the original budget."""
    if (
        type(originally_admitted_mib) is not int
        or originally_admitted_mib < MIN_STAGE_BUDGET_MIB
        or status.get("state") != "cuda_detected"
        or status.get("admission_state") != "admitted"
    ):
        raise RuntimeError("my_voice:gpu_admission_invalid")
    latest = status.get("safe_budget_mib")
    if type(latest) is not int or latest < MIN_STAGE_BUDGET_MIB:
        raise RuntimeError("my_voice:gpu_headroom_insufficient")
    return min(latest, originally_admitted_mib)
