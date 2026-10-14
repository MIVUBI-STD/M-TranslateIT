"""Read-only My Voice device discovery for the selected local PyTorch runtime.

Enumerates the CUDA devices *visible to the user's environment*, matches
each physical GPU UUID to nvidia-smi, and chooses a device with safe headroom.
Model names and NVIDIA index 0 are never used as proof of device identity.
All threshold decisions are provisional; no hardware training is certified.
"""
from __future__ import annotations

import json
import os
import platform
from typing import Any, Callable

from my_voice_gpu_identity import (
    MAX_VISIBLE_CUDA_DEVICES, DeviceIdentityError, cuda_device_uuid,
    normalize_uuid, sensor_inventory_mib,
)
from my_voice_gpu_policy import PROFILE, assess_memory

MIB = 1024 * 1024


def empty_report(cpu_name: str) -> dict[str, Any]:
    return {
        "state": "unavailable",
        "cpu_name": cpu_name[:200] or "CPU name unavailable",
        "gpu_name": None,
        "gpu_uuid": None,
        "cuda_index": None,
        "visible_cuda_devices": 0,
        "vram_mib": None,
        "free_vram_mib": None,
        "reserve_mib": None,
        "safe_budget_mib": None,
        "admission_state": "unknown",
        "profile": PROFILE,
        "detail": "GPU hardware has not been verified.",
    }


def detect_compute(
    torch_module: Any, cpu_name: str, *,
    required_uuid: str | None = None,
    sensor_reader: Callable[[str], tuple[int, int]] | None = None,
) -> dict[str, Any]:
    result = empty_report(cpu_name)
    try:
        cuda = torch_module.cuda
        if not bool(cuda.is_available()):
            result["state"] = "cpu_only"
            result["detail"] = (
                "No supported CUDA device is available in this Python runtime. "
                "My Voice CPU training is not an automatic fallback."
            )
            return result
        count = int(cuda.device_count())
        result["visible_cuda_devices"] = count
        if not 0 < count <= MAX_VISIBLE_CUDA_DEVICES:
            raise DeviceIdentityError("unsupported_cuda_device_count")
        target = normalize_uuid(required_uuid) if required_uuid else None
        # One NVIDIA inventory call regardless of the number of CUDA cards.
        if sensor_reader is None:
            inventory = sensor_inventory_mib()
            sensor_reader = lambda uuid: inventory[uuid]
        candidates: list[dict[str, Any]] = []
        seen: set[str] = set()
        for index in range(count):
            try:
                props = cuda.get_device_properties(index)
                uuid = cuda_device_uuid(cuda, index)
                if uuid in seen:
                    raise DeviceIdentityError("duplicate_cuda_uuid")
                seen.add(uuid)
                if target is not None and uuid != target:
                    continue
                name = str(cuda.get_device_name(index)).strip()
                total = int(props.total_memory)
                cuda_free, cuda_total = cuda.mem_get_info(index)
                smi_total_mib, smi_free_mib = sensor_reader(uuid)
                if (
                    not name or total <= 0
                    or type(cuda_free) is not int or type(cuda_total) is not int
                    or abs(cuda_total - total) > 256 * MIB
                    or abs(smi_total_mib * MIB - total) > max(1024 * MIB, total // 8)
                ):
                    raise DeviceIdentityError("cuda_nvidia_memory_identity_mismatch")
                # UUID, not a potentially different driver memory report, is
                # the device identity; use lower free reading conservatively.
                free = min(cuda_free, smi_free_mib * MIB)
                policy = assess_memory(total, free)
                candidates.append({
                    "name": name[:200], "uuid": uuid, "index": index,
                    "vram_mib": total // MIB,
                    "free_vram_mib": free // MIB,
                    "reserve_mib": policy["reserve_mib"],
                    "safe_budget_mib": policy["safe_budget_mib"],
                    "admission_state": policy["state"],
                })
            except DeviceIdentityError as exc:
                if str(exc) == "duplicate_cuda_uuid" or target is not None:
                    raise
                # Other GPUs may be incompatible or unavailable. Never
                # select them; continue with any independently verified GPU.
                continue
            except (OSError, RuntimeError, ValueError, AttributeError, TypeError):
                if target is not None:
                    raise
                continue
        if not candidates:
            raise DeviceIdentityError("requested_cuda_uuid_missing")
        # Prefer a device that passes admission, then the most usable budget.
        # UUID provides a stable tie-break instead of CUDA/NVIDIA ordinal.
        best = sorted(
            candidates,
            key=lambda x: (
                0 if x["admission_state"] == "admitted" else 1,
                -int(x["safe_budget_mib"] or 0), -int(x["vram_mib"]), x["uuid"],
            ),
        )[0]
        result.update({
            "state": "cuda_detected",
            "gpu_name": best["name"],
            "gpu_uuid": best["uuid"],
            "cuda_index": best["index"],
            "vram_mib": best["vram_mib"],
            "free_vram_mib": best["free_vram_mib"],
            "reserve_mib": best["reserve_mib"],
            "safe_budget_mib": best["safe_budget_mib"],
            "admission_state": best["admission_state"],
            "detail": (
                "Physical CUDA GPU selected by verified NVIDIA UUID. "
                "Budget is provisional; successful training is unverified."
                if best["admission_state"] == "admitted" else
                "CUDA GPU detected but the current free memory is below "
                "the safe training admission budget."
            ),
        })
    except Exception:
        result["state"] = "unavailable"
        result["admission_state"] = "unknown"
        result["gpu_name"] = None
        result["gpu_uuid"] = None
        result["cuda_index"] = None
        result["vram_mib"] = None
        result["free_vram_mib"] = None
        result["reserve_mib"] = None
        result["safe_budget_mib"] = None
        result["detail"] = (
            "Cannot independently verify CUDA GPU identity and memory against "
            "the NVIDIA sensor. Training is blocked; check the GPU driver and "
            "installed Python runtime."
        )
    return result


def main() -> None:
    cpu = platform.processor() or platform.machine() or "CPU name unavailable"
    try:
        import torch
    except Exception:
        result = empty_report(cpu)
        result["detail"] = "PyTorch could not load from the local runtime."
    else:
        result = detect_compute(
            torch, cpu, required_uuid=os.environ.get("TRANSLATEIT_MYVOICE_GPU_UUID"),
        )
    print(json.dumps(result, separators=(",", ":"), ensure_ascii=True))


if __name__ == "__main__":
    main()
