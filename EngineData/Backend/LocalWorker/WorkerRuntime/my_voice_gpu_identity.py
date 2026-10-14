"""One physical NVIDIA GPU identity across CUDA, nvidia-smi and trainers.

NVIDIA UUID is the immutable per-run device locator. A CUDA ordinal or a
friendly model name is not enough on multi-GPU systems. No GPU is accepted
when UUID cannot be read and matched to NVIDIA sensor data.
"""
from __future__ import annotations

import os
import re
import shutil
import subprocess
from pathlib import Path
from typing import Any

_UUID = re.compile(r"^GPU-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$", re.I)
MAX_VISIBLE_CUDA_DEVICES = 8
SENSOR_TIMEOUT = 3


class DeviceIdentityError(RuntimeError):
    pass


def normalize_uuid(raw: Any) -> str:
    """Accept a full NVIDIA UUID or PyTorch CUuuid bytes, never a GPU name."""
    if hasattr(raw, "bytes"):
        octets = bytes(raw.bytes)
        if len(octets) != 16:
            raise DeviceIdentityError("my_voice:cuda_uuid_invalid_bytes")
        value = octets.hex()
    else:
        value = str(raw).strip().upper()
        if value.startswith("GPU-"):
            value = value[4:]
        value = value.replace("-", "")
    if not re.fullmatch(r"[0-9A-Fa-f]{32}", value):
        raise DeviceIdentityError("my_voice:cuda_uuid_missing_or_invalid")
    value = value.lower()
    result = "GPU-" + "-".join((
        value[:8], value[8:12], value[12:16], value[16:20], value[20:]
    ))
    if not _UUID.fullmatch(result):
        raise DeviceIdentityError("my_voice:cuda_uuid_invalid")
    return result


def cuda_device_uuid(cuda: Any, index: int) -> str:
    props = cuda.get_device_properties(index)
    identity = getattr(props, "uuid", None)
    if identity is None:
        raise DeviceIdentityError("my_voice:pytorch_cuda_uuid_not_supported")
    return normalize_uuid(identity)


def sensor_executable() -> str:
    found = shutil.which("nvidia-smi")
    if found:
        return found
    if os.name == "nt":
        candidate = Path(os.environ.get("ProgramFiles", "C:/Program Files")) / (
            "NVIDIA Corporation/NVSMI/nvidia-smi.exe"
        )
        if candidate.is_file():
            return str(candidate)
    raise DeviceIdentityError("my_voice:nvidia_gpu_sensor_unavailable")


def parse_sensor_row(output: str, requested_uuid: str) -> tuple[int, int]:
    """Read MiB from the exact requested physical GPU; reject wrong UUID."""
    expected = normalize_uuid(requested_uuid)
    rows = output.strip().splitlines()
    if len(rows) != 1:
        raise DeviceIdentityError("my_voice:nvidia_sensor_row_count_invalid")
    columns = [part.strip() for part in rows[0].split(",")]
    if len(columns) != 3 or normalize_uuid(columns[0]) != expected:
        raise DeviceIdentityError("my_voice:nvidia_sensor_gpu_identity_mismatch")
    if not columns[1].isdecimal() or not columns[2].isdecimal():
        raise DeviceIdentityError("my_voice:nvidia_sensor_memory_unreadable")
    total, free = int(columns[1]), int(columns[2])
    if not 0 < total <= 1048576 or not 0 <= free <= total:
        raise DeviceIdentityError("my_voice:nvidia_sensor_memory_invalid")
    return total, free


def sensor_memory_mib(uuid: str, executable: str | None = None) -> tuple[int, int]:
    selected = normalize_uuid(uuid)
    executable = executable or sensor_executable()
    try:
        response = subprocess.run(
            [
                executable, "--id=" + selected,
                "--query-gpu=uuid,memory.total,memory.free",
                "--format=csv,noheader,nounits",
            ],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, text=True, check=False,
            timeout=SENSOR_TIMEOUT,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise DeviceIdentityError("my_voice:nvidia_sensor_timeout_or_failed") from exc
    if response.returncode != 0 or len(response.stdout) > 512:
        raise DeviceIdentityError("my_voice:nvidia_sensor_unavailable")
    return parse_sensor_row(response.stdout, selected)
