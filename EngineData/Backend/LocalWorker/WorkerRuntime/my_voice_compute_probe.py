"""Read-only, bounded My Voice compute advisory for an explicit UI check.

A detected CUDA device is not proof of successful training, free VRAM,
model compatibility, or realtime inference. Never change torch global state.
"""
from __future__ import annotations

import json
import platform
from typing import Any

from my_voice_gpu_policy import PROFILE, assess_memory


def detect_compute(torch_module: Any, cpu_name: str) -> dict[str, Any]:
    result: dict[str, Any] = {
        "state": "unavailable",
        "cpu_name": cpu_name[:200] or "CPU name unavailable",
        "gpu_name": None,
        "vram_mib": None,
        "free_vram_mib": None,
        "reserve_mib": None,
        "safe_budget_mib": None,
        "admission_state": "unknown",
        "profile": PROFILE,
        "detail": "Compute status could not be verified.",
    }
    try:
        if not bool(torch_module.cuda.is_available()):
            result["state"] = "cpu_only"
            result["detail"] = (
                "CUDA is not available in this Python runtime. CPU-only "
                "My Voice training is unverified and may be slow or fail."
            )
            return result
        properties = torch_module.cuda.get_device_properties(0)
        name = str(torch_module.cuda.get_device_name(0)).strip()
        vram_bytes = int(properties.total_memory)
        if not name or vram_bytes <= 0:
            raise ValueError("invalid_cuda_device_details")
        free_bytes, reported_total = torch_module.cuda.mem_get_info(0)
        if not isinstance(free_bytes, int) or not isinstance(reported_total, int):
            raise ValueError("invalid_cuda_free_memory")
        # PyTorch device total and CUDA mem_get_info total may differ slightly.
        if abs(reported_total - vram_bytes) > 256 * 1024 * 1024:
            raise ValueError("cuda_total_memory_mismatch")
        admission = assess_memory(vram_bytes, free_bytes)
        result["state"] = "cuda_detected"
        result["gpu_name"] = name[:200]
        result["vram_mib"] = vram_bytes // (1024 * 1024)
        result["free_vram_mib"] = free_bytes // (1024 * 1024)
        result.update(admission)
        result["admission_state"] = result.pop("state")
        result["state"] = "cuda_detected"
        result["detail"] = (
            "Safe headroom is an initial unverified policy, not a memory "
            "reservation or proof that GPT-SoVITS will fit."
            if result["admission_state"] == "admitted"
            else "Insufficient free VRAM for the conservative training profile."
        )
    except Exception:
        result["detail"] = (
            "CUDA device inspection failed. Repair or verify the GPU driver "
            "and packaged Python runtime before starting training."
        )
    return result


def main() -> None:
    cpu_name = platform.processor() or platform.machine() or "CPU name unavailable"
    try:
        import torch
    except Exception:
        result = {
            "state": "unavailable",
            "cpu_name": cpu_name[:200],
            "gpu_name": None,
            "vram_mib": None,
            "free_vram_mib": None,
            "reserve_mib": None,
            "safe_budget_mib": None,
            "admission_state": "unknown",
            "profile": PROFILE,
            "detail": "PyTorch could not be loaded from the selected local Python runtime.",
        }
    else:
        result = detect_compute(torch, cpu_name)
    print(json.dumps(result, separators=(",", ":"), ensure_ascii=True))


if __name__ == "__main__":
    main()
