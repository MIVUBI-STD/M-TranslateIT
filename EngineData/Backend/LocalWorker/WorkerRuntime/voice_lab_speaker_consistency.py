"""Advisory speaker consistency from the existing GPT-SoVITS SV stage.

No new model is loaded. These relative cosines are NOT an absolute speaker
identity score, an automatic take-rejection policy, or a quality guarantee.
"""

from __future__ import annotations

import math
from pathlib import Path
from typing import Any

from voice_lab_gpt_sovits import VoiceLabProviderError

MAX_TAKES = 128
MAX_EMBEDDING_BYTES = 2 * 1024 * 1024
MAX_EMBEDDING_DIMENSION = 65_536


def inspect_speaker_embeddings(
    embedding_dir: Path, takes: list[dict[str, Any]]
) -> dict[str, Any]:
    if not 2 <= len(takes) <= MAX_TAKES:
        raise VoiceLabProviderError("speaker_consistency_invalid_take_count")

    import torch  # Existing pinned GPT-SoVITS dependency, used only during build.

    normalized = []
    dimension: int | None = None
    seen: set[int] = set()
    for take in takes:
        line_id = int(take["line_id"])
        name = str(take["wav_file"])
        if line_id <= 0 or line_id in seen or name != f"take_{line_id:04}.wav":
            raise VoiceLabProviderError("speaker_consistency_invalid_take")
        seen.add(line_id)
        source = embedding_dir / f"{name}.pt"
        if source.is_symlink() or not source.is_file():
            raise VoiceLabProviderError(f"speaker_embedding_missing:{line_id}")
        if not 0 < source.stat().st_size <= MAX_EMBEDDING_BYTES:
            raise VoiceLabProviderError(f"speaker_embedding_invalid_size:{line_id}")
        try:
            tensor = torch.load(source, map_location="cpu", weights_only=True)
        except (OSError, RuntimeError, ValueError) as exc:
            raise VoiceLabProviderError(f"speaker_embedding_unreadable:{line_id}") from exc
        if not torch.is_tensor(tensor):
            raise VoiceLabProviderError(f"speaker_embedding_invalid_type:{line_id}")
        vector = tensor.detach().float().reshape(-1)
        if not 0 < vector.numel() <= MAX_EMBEDDING_DIMENSION:
            raise VoiceLabProviderError(f"speaker_embedding_invalid_shape:{line_id}")
        if dimension is not None and vector.numel() != dimension:
            raise VoiceLabProviderError(f"speaker_embedding_dimension_mismatch:{line_id}")
        if not bool(torch.isfinite(vector).all()):
            raise VoiceLabProviderError(f"speaker_embedding_nonfinite:{line_id}")
        magnitude = float(torch.linalg.vector_norm(vector))
        if not math.isfinite(magnitude) or magnitude <= 1e-8:
            raise VoiceLabProviderError(f"speaker_embedding_zero_norm:{line_id}")
        dimension = vector.numel()
        normalized.append(vector / magnitude)

    matrix = torch.stack(normalized)
    pairwise = matrix @ matrix.T
    per_take = []
    for row, take in enumerate(takes):
        peers = sorted(
            float(pairwise[row, peer])
            for peer in range(len(takes))
            if peer != row
        )
        midpoint = len(peers) // 2
        median = (
            peers[midpoint]
            if len(peers) % 2
            else (peers[midpoint - 1] + peers[midpoint]) / 2
        )
        per_take.append({
            "line_id": int(take["line_id"]),
            "median_peer_cosine": round(median, 6),
            "minimum_peer_cosine": round(peers[0], 6),
        })
    return {
        "method": "pinned_gpt_sovits_peer_embedding_cosine",
        "advisory_only": True,
        "automated_take_rejection": False,
        "per_take": per_take,
    }
