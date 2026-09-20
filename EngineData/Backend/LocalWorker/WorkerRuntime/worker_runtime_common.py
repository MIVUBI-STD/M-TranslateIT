from __future__ import annotations

import os
import time
from pathlib import Path
from typing import Any

import translation_envelope  # noqa: F401 - dynamic provider export

SCRIPT_ROOT = Path(__file__).resolve().parents[4]


def configured_absolute_root(environment_name: str, fallback: Path) -> Path:
    raw = str(os.environ.get(environment_name, "")).strip()
    if not raw:
        return fallback.resolve()
    candidate = Path(raw)
    if not candidate.is_absolute():
        raise RuntimeError(f"worker:{environment_name.lower()}_must_be_absolute")
    return candidate.resolve()


RUNTIME_ROOT = configured_absolute_root("TRANSLATEIT_RUNTIME_ROOT", SCRIPT_ROOT)
USER_DATA_ROOT = configured_absolute_root("TRANSLATEIT_USER_DATA_ROOT", SCRIPT_ROOT / "UserData")
RUNTIME_ASSETS_ROOT = RUNTIME_ROOT / "EngineData" / "Backend" / "RuntimeAssets"
ASR_MODEL_ROOT = RUNTIME_ASSETS_ROOT / "ASR" / "ModelData"
TRANSLATION_MODEL_ROOT = RUNTIME_ASSETS_ROOT / "Translation" / "ModelData"
GPT_SOVITS_SOURCE_ROOT = RUNTIME_ASSETS_ROOT / "Voice" / "GPTSoVITS" / "Source"
VOICE_ACTOR_ROOT = USER_DATA_ROOT / "SavedProject" / "VoiceLab" / "MyVoice"
CACHE_ROOT = USER_DATA_ROOT / "CacheData"
LOG_ROOT = USER_DATA_ROOT / "LogData"
ALLOWED_INPUT_ROOTS = [CACHE_ROOT, LOG_ROOT]
ALLOWED_OUTPUT_ROOTS = [CACHE_ROOT]

MAX_WORKER_REQUEST_BYTES = 1_000_000
MAX_TRANSLATION_TEXT_CHARS = 2_000
MAX_TTS_TEXT_CHARS = 1_000
MAX_TRANSCRIPT_TEXT_CHARS = 4_000
MAX_AUDIO_INPUT_BYTES = 25 * 1024 * 1024
MAX_REASONABLE_MODEL_TOKEN_LIMIT = 1_000_000

_GPU_CAPABILITY_SNAPSHOT: dict[str, Any] | None = None
_GPU_CAPABILITY_PROBE_OWNER: tuple[object, object] | None = None
_GPU_CAPABILITY_PROBE_COUNT = 0


def now_ms() -> int:
    return int(time.time() * 1000)


def request_deadline_remaining_ms(payload: dict[str, Any] | None) -> int | None:
    if not isinstance(payload, dict):
        return None
    try:
        deadline = int(payload.get("deadline_unix_ms", 0))
    except (TypeError, ValueError):
        return None
    if deadline <= 0:
        return None
    return max(0, deadline - now_ms())


def request_deadline_expired(payload: dict[str, Any] | None) -> bool:
    remaining = request_deadline_remaining_ms(payload)
    return remaining is not None and remaining <= 0


def import_ready(module_name: str) -> bool:
    try:
        __import__(module_name)
        return True
    except Exception:
        return False


def bounded_int(value: Any, fallback: int, minimum: int, maximum: int) -> int:
    try:
        parsed = int(value)
    except Exception:
        parsed = fallback
    return max(minimum, min(maximum, parsed))


def bounded_float(value: Any, fallback: float, minimum: float, maximum: float) -> float:
    try:
        parsed = float(value)
    except Exception:
        parsed = fallback
    return max(minimum, min(maximum, parsed))


def compact_runtime_text(value: Any, max_chars: int) -> str:
    text = str(value or "").strip().replace("\x00", "")
    compact = " ".join(text.split())
    return compact[:max_chars] if len(compact) > max_chars else compact


def runtime_text_too_large(value: Any, max_chars: int) -> bool:
    return len(str(value or "")) > max_chars


def safe_command_name(value: Any) -> str:
    text = str(value or "status").strip().lower()
    return "".join(ch for ch in text if ch.isascii() and (ch.isalnum() or ch == "_"))[:64]


def torch_status() -> dict[str, Any]:
    try:
        import torch
    except Exception as exc:
        return {
            "import_ready": False,
            "cuda_probe_ok": False,
            "cuda_available": False,
            "blocker": f"dependency:torch_import_failed:{type(exc).__name__}",
        }
    try:
        available = bool(torch.cuda.is_available())
    except Exception as exc:
        return {
            "import_ready": True,
            "cuda_probe_ok": False,
            "cuda_available": False,
            "blocker": f"cuda:torch_probe_failed:{type(exc).__name__}",
        }
    return {"import_ready": True, "cuda_probe_ok": True, "cuda_available": available, "blocker": ""}


def ctranslate2_status() -> dict[str, Any]:
    try:
        import ctranslate2
    except Exception as exc:
        return {
            "import_ready": False,
            "cuda_probe_ok": False,
            "cuda_available": False,
            "blocker": f"dependency:ctranslate2_import_failed:{type(exc).__name__}",
        }
    probe = getattr(ctranslate2, "get_cuda_device_count", None)
    if not callable(probe):
        return {
            "import_ready": True,
            "cuda_probe_ok": False,
            "cuda_available": False,
            "blocker": "cuda:ctranslate2_probe_unavailable",
        }
    try:
        available = int(probe()) > 0
    except Exception as exc:
        return {
            "import_ready": True,
            "cuda_probe_ok": False,
            "cuda_available": False,
            "blocker": f"cuda:ctranslate2_probe_failed:{type(exc).__name__}",
        }
    return {"import_ready": True, "cuda_probe_ok": True, "cuda_available": available, "blocker": ""}


def clear_gpu_capability_snapshot() -> None:
    global _GPU_CAPABILITY_SNAPSHOT, _GPU_CAPABILITY_PROBE_OWNER
    _GPU_CAPABILITY_SNAPSHOT = None
    _GPU_CAPABILITY_PROBE_OWNER = None


def probe_gpu_runtime(payload: dict[str, Any] | None = None) -> dict[str, Any]:
    del payload
    global _GPU_CAPABILITY_SNAPSHOT, _GPU_CAPABILITY_PROBE_OWNER, _GPU_CAPABILITY_PROBE_COUNT
    owner = (torch_status, ctranslate2_status)
    if _GPU_CAPABILITY_SNAPSHOT is not None and _GPU_CAPABILITY_PROBE_OWNER == owner:
        return dict(_GPU_CAPABILITY_SNAPSHOT)

    torch_probe = torch_status()
    ct2_probe = ctranslate2_status()
    _GPU_CAPABILITY_PROBE_COUNT += 1
    torch_ready = bool(torch_probe["import_ready"])
    torch_probe_ok = bool(torch_probe["cuda_probe_ok"])
    torch_cuda = bool(torch_probe["cuda_available"])
    ct2_ready = bool(ct2_probe["import_ready"])
    ct2_probe_ok = bool(ct2_probe["cuda_probe_ok"])
    ct2_cuda = bool(ct2_probe["cuda_available"])
    known = torch_ready and torch_probe_ok and ct2_ready and ct2_probe_ok
    cpu_fallback = known and (not torch_cuda or not ct2_cuda)
    selected_device = "cuda" if ct2_cuda else "cpu" if ct2_ready and ct2_probe_ok else "blocked"
    selected_compute = (
        "int8_float16"
        if selected_device == "cuda"
        else "int8"
        if selected_device == "cpu"
        else "blocked"
    )
    selected_translation = (
        "cuda" if torch_cuda else "cpu" if torch_ready and torch_probe_ok else "blocked"
    )
    blockers = [b for b in (torch_probe["blocker"], ct2_probe["blocker"]) if b]
    snapshot = {
        "torch_import_ready": torch_ready,
        "torch_cuda_probe_ok": torch_probe_ok,
        "torch_cuda_available": torch_cuda,
        "torch_cuda_probe_blocker": str(torch_probe["blocker"]),
        "ctranslate2_import_ready": ct2_ready,
        "ctranslate2_cuda_probe_ok": ct2_probe_ok,
        "ctranslate2_cuda_available": ct2_cuda,
        "ctranslate2_cuda_probe_blocker": str(ct2_probe["blocker"]),
        "cuda_capability_known": known,
        "cpu_fallback_active": cpu_fallback,
        "cuda_probe_blocker": ";".join(blockers),
        "cuda_primary_requested": True,
        "selected_device": selected_device,
        "selected_translation_device": selected_translation,
        "selected_compute_type": selected_compute,
        "fallback_reason": "cuda_unavailable" if cpu_fallback else "",
        "capability_probe_count": _GPU_CAPABILITY_PROBE_COUNT,
    }
    _GPU_CAPABILITY_SNAPSHOT = snapshot
    _GPU_CAPABILITY_PROBE_OWNER = owner
    return dict(snapshot)


def normalize_language(value: Any, fallback: str) -> str:
    text = str(value or fallback).strip().lower().replace("_latn", "")
    if text.startswith("ind") or text == "id":
        return "id"
    if text.startswith("eng") or text == "en":
        return "en"
    return text[:2] if text else fallback


def direction_pair(source_language: str, target_language: str) -> str:
    return (
        f"{normalize_language(source_language, 'id')}->{normalize_language(target_language, 'en')}"
    )


def translation_runtime_config() -> tuple[str, str]:
    probe = torch_status()
    if not probe["import_ready"]:
        raise RuntimeError(str(probe["blocker"] or "dependency:torch_missing"))
    if not probe["cuda_probe_ok"]:
        raise RuntimeError(str(probe["blocker"] or "cuda:torch_capability_unknown"))
    return ("cuda", "") if probe["cuda_available"] else ("cpu", "torch_cuda_unavailable")


def move_inputs_to_device(inputs: Any, device: str) -> Any:
    if device != "cuda":
        return inputs
    return {key: value.to("cuda") for key, value in inputs.items()}


def input_token_count(inputs: Any) -> int | None:
    try:
        ids = inputs["input_ids"]
        shape = getattr(ids, "shape", None)
        if shape is not None and len(shape) >= 2:
            return int(shape[-1])
        if isinstance(ids, (list, tuple)):
            return len(ids[0]) if ids and isinstance(ids[0], (list, tuple)) else len(ids)
    except Exception:
        return None
    return None


def normalized_token_id_set(value: Any) -> set[int]:
    values = value if isinstance(value, (list, tuple, set)) else ([] if value is None else [value])
    result: set[int] = set()
    for item in values:
        try:
            token = int(item)
        except Exception:
            continue
        if token >= 0:
            result.add(token)
    return result


def generation_eos_token_ids(tokenizer: Any, model: Any) -> set[int]:
    ids = normalized_token_id_set(getattr(tokenizer, "eos_token_id", None))
    ids.update(
        normalized_token_id_set(
            getattr(getattr(model, "generation_config", None), "eos_token_id", None)
        )
    )
    ids.update(
        normalized_token_id_set(getattr(getattr(model, "config", None), "eos_token_id", None))
    )
    return ids


def generation_pad_token_ids(tokenizer: Any, model: Any) -> set[int]:
    ids = normalized_token_id_set(getattr(tokenizer, "pad_token_id", None))
    ids.update(
        normalized_token_id_set(
            getattr(getattr(model, "generation_config", None), "pad_token_id", None)
        )
    )
    ids.update(
        normalized_token_id_set(getattr(getattr(model, "config", None), "pad_token_id", None))
    )
    return ids


def resolve_worker_path(value: Any, default_path: Path, allowed_roots: list[Path]) -> Path:
    raw = str(value).strip() if value not in (None, "") else str(default_path)
    path = Path(raw)
    if not path.is_absolute():
        normalized = raw.replace("\\", "/")
        if normalized == "UserData" or normalized.startswith("UserData/"):
            path = USER_DATA_ROOT / normalized.removeprefix("UserData").lstrip("/")
        else:
            path = RUNTIME_ROOT / path
    resolved = path.resolve()
    allowed = [root.resolve() for root in allowed_roots]
    if not any(resolved == root or root in resolved.parents for root in allowed):
        raise ValueError("worker:path_outside_allowed_roots")
    return resolved
