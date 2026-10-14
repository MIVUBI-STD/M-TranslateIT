"""Read-only, bounded My Voice compute advisory for an explicit UI check.

A detected CUDA device is not proof of successful training, free VRAM,
model compatibility, or realtime inference. Never change torch global state.
"""
from __future__ import annotations

import json
import platform
from typing import Any


def detect_compute(torch_module: Any, cpu_name: str) -> dict[str, Any]:
    result: dict[str, Any] = {
        "state": "unavailable",
        "cpu_name": cpu_name[:200] or "CPU name unavailable",
        "gpu_name": None,
        "vram_mib": None,
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
        result["state"] = "cuda_detected"
        result["gpu_name"] = name[:200]
        result["vram_mib"] = vram_bytes // (1024 * 1024)
        result["detail"] = (
            "CUDA device detected. Successful My Voice training, available "
            "VRAM, and voice quality have not yet been verified."
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
            "detail": "PyTorch could not be loaded from the selected local Python runtime.",
        }
    else:
        result = detect_compute(torch, cpu_name)
    print(json.dumps(result, separators=(",", ":"), ensure_ascii=True))


if __name__ == "__main__":
    main()
