"""Conservative device-capability eligibility for pinned GPT-SoVITS mixed FP16.

Compute >=7 is a provisional, explicit software policy. Older cards may
theoretically support other precisions, but they are NOT silently converted
to an unreviewed, more memory-intensive FP32 pipeline.
"""

from __future__ import annotations

MIN_MIXED_FP16_MAJOR = 7


def device_precision_policy(cuda: object, index: int) -> tuple[str, str]:
    major, minor = cuda.get_device_capability(index)
    if (
        type(major) is not int
        or type(minor) is not int
        or not 1 <= major <= 20
        or not 0 <= minor <= 9
    ):
        raise RuntimeError("my_voice:compute_capability_invalid")
    return (
        f"{major}.{minor}",
        "mixed_fp16" if major >= MIN_MIXED_FP16_MAJOR else "unsupported_pinned_precision",
    )
