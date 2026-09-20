from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import voice_lab_gpt_sovits as voice_actor_provider
import worker_runtime_common as common

ASR_MODEL = common.ASR_MODEL_ROOT / "faster-whisper-large-v3-turbo"
ASR_BACKUP_MODEL = common.ASR_MODEL_ROOT / "faster-whisper-medium"
ASR_RUNTIME: Any | None = None
ASR_RUNTIME_DEVICE = "not_loaded"
ASR_RUNTIME_COMPUTE = "not_loaded"
ASR_RUNTIME_MODEL_ID = "not_loaded"
MAX_ASR_HOTWORDS_CHARS = 1024
VOICE_ACTOR_RUNTIME: dict[str, Any] | None = None
VOICE_ACTOR_RUNTIME_FINGERPRINT: Any | None = None


def has_any(path: Path, patterns: tuple[str, ...]) -> bool:
    return path.is_dir() and any(
        any(item.is_file() for item in path.glob(pattern)) for pattern in patterns
    )


def asr_model_ready(path: Path) -> bool:
    return (
        (path / "model.bin").is_file()
        and (path / "config.json").is_file()
        and has_any(path, ("tokenizer.json", "tokenizer.model", "vocabulary.json"))
    )


def choose_asr_model() -> tuple[str, Path]:
    if asr_model_ready(ASR_MODEL):
        return "faster-whisper-large-v3-turbo", ASR_MODEL
    if asr_model_ready(ASR_BACKUP_MODEL):
        return "faster-whisper-medium", ASR_BACKUP_MODEL
    return "faster-whisper-large-v3-turbo", ASR_MODEL


def asr_runtime_config(payload: dict[str, Any] | None = None) -> tuple[str, str, str]:
    gpu = common.probe_gpu_runtime(payload)
    selected = str(gpu["selected_device"])
    if selected == "blocked":
        raise RuntimeError(str(gpu["cuda_probe_blocker"] or "cuda:ctranslate2_capability_unknown"))
    if selected == "cuda":
        return "cuda", "int8_float16", ""
    return "cpu", "int8", str(gpu["fallback_reason"])


def asr_blocker(exc: Exception) -> str:
    detail = str(exc).strip()
    if detail:
        safe = "".join(ch for ch in detail if ch.isascii() and (ch.isalnum() or ch in "_:-"))
        if safe.startswith(("asr:", "cuda:", "dependency:", "model:", "worker:")):
            return safe[:160]
    return f"asr:runtime_failed:{type(exc).__name__}"


def get_asr_runtime(payload: dict[str, Any] | None = None) -> Any:
    global ASR_RUNTIME, ASR_RUNTIME_DEVICE, ASR_RUNTIME_COMPUTE, ASR_RUNTIME_MODEL_ID
    if ASR_RUNTIME is not None:
        return ASR_RUNTIME
    from faster_whisper import WhisperModel

    device, compute_type, _fallback = asr_runtime_config(payload)
    model_id, model_path = choose_asr_model()
    ASR_RUNTIME = WhisperModel(str(model_path), device=device, compute_type=compute_type)
    ASR_RUNTIME_DEVICE = device
    ASR_RUNTIME_COMPUTE = compute_type
    ASR_RUNTIME_MODEL_ID = model_id
    return ASR_RUNTIME


def handle_asr_preload(payload: dict[str, Any]) -> dict[str, Any]:
    started = common.now_ms()
    model_id, model_path = choose_asr_model()
    if not common.import_ready("faster_whisper"):
        return {
            "ok": False,
            "stage": "asr_preload",
            "model_id": model_id,
            "model_path": str(model_path),
            "blocker": "dependency:faster_whisper_missing",
            "elapsed_ms": common.now_ms() - started,
        }
    if not asr_model_ready(model_path):
        return {
            "ok": False,
            "stage": "asr_preload",
            "model_id": model_id,
            "model_path": str(model_path),
            "blocker": "model:faster_whisper_large_v3_turbo_missing",
            "elapsed_ms": common.now_ms() - started,
        }
    try:
        get_asr_runtime(payload)
        return {
            "ok": True,
            "stage": "asr_preload",
            "model_id": ASR_RUNTIME_MODEL_ID,
            "model_path": str(model_path),
            "device": ASR_RUNTIME_DEVICE,
            "compute_type": ASR_RUNTIME_COMPUTE,
            "fallback_reason": (
                "" if ASR_RUNTIME_DEVICE == "cuda" else "cuda_unavailable_cpu_fallback_active"
            ),
            "elapsed_ms": common.now_ms() - started,
        }
    except Exception as exc:
        return {
            "ok": False,
            "stage": "asr_preload",
            "blocker": asr_blocker(exc),
            "note": str(exc),
            "elapsed_ms": common.now_ms() - started,
        }


def handle_transcribe(payload: dict[str, Any]) -> dict[str, Any]:
    started = common.now_ms()
    try:
        audio_path = common.resolve_worker_path(
            payload.get("audio_path", ""),
            common.CACHE_ROOT / "audio_segments" / "latest_live_target_segment.wav",
            common.ALLOWED_INPUT_ROOTS,
        )
    except Exception as exc:
        return {
            "ok": False,
            "stage": "transcribe",
            "blocker": asr_blocker(exc),
            "note": str(exc),
        }
    if not audio_path.is_file():
        return {
            "ok": False,
            "stage": "transcribe",
            "blocker": "asr:audio_file_missing",
            "audio_path": str(audio_path),
        }
    if audio_path.stat().st_size > common.MAX_AUDIO_INPUT_BYTES:
        return {
            "ok": False,
            "stage": "transcribe",
            "blocker": "asr:audio_file_too_large",
            "max_bytes": common.MAX_AUDIO_INPUT_BYTES,
        }
    try:
        model = get_asr_runtime(payload)
        hotwords = common.compact_runtime_text(payload.get("hotwords", ""), MAX_ASR_HOTWORDS_CHARS)
        segments, info = model.transcribe(
            str(audio_path),
            language=common.normalize_language(payload.get("language", "id"), "id"),
            task="transcribe",
            beam_size=common.bounded_int(payload.get("beam_size", 1), 1, 1, 5),
            temperature=common.bounded_float(payload.get("temperature", 0), 0.0, 0.0, 1.0),
            condition_on_previous_text=False,
            vad_filter=bool(payload.get("vad_filter", True)),
            without_timestamps=True,
            word_timestamps=False,
            hotwords=hotwords or None,
        )
        text = common.compact_runtime_text(
            " ".join(segment.text.strip() for segment in segments),
            common.MAX_TRANSCRIPT_TEXT_CHARS,
        )
        return {
            "ok": bool(text),
            "stage": "transcribe",
            "transcript_text": text,
            "language": getattr(info, "language", "id"),
            "language_probability": float(getattr(info, "language_probability", 0.0)),
            "device": ASR_RUNTIME_DEVICE,
            "compute_type": ASR_RUNTIME_COMPUTE,
            "model_id": ASR_RUNTIME_MODEL_ID,
            "hotwords_applied": bool(hotwords),
            "elapsed_ms": common.now_ms() - started,
            "blocker": "" if text else "asr:empty_transcript",
        }
    except Exception as exc:
        return {
            "ok": False,
            "stage": "transcribe",
            "blocker": asr_blocker(exc),
            "note": str(exc),
            "elapsed_ms": common.now_ms() - started,
        }


def clear_voice_actor_runtime() -> None:
    global VOICE_ACTOR_RUNTIME, VOICE_ACTOR_RUNTIME_FINGERPRINT
    VOICE_ACTOR_RUNTIME = None
    VOICE_ACTOR_RUNTIME_FINGERPRINT = None


def voice_actor_blocker(exc: Exception) -> str:
    detail = str(exc).strip()
    if isinstance(exc, voice_actor_provider.VoiceLabProviderError) and detail:
        safe = "".join(ch for ch in detail if ch.isascii() and (ch.isalnum() or ch in "_:-"))
        if safe:
            return f"voice_actor:{safe[:160]}"
    return f"voice_actor:runtime_failed:{type(exc).__name__}"


def voice_actor_package_token(package: dict[str, Any]) -> str:
    fingerprint = package.get("fingerprint")
    if not isinstance(fingerprint, tuple) or not fingerprint:
        raise voice_actor_provider.VoiceLabProviderError("actor_identity_missing")
    token = json.dumps(fingerprint, ensure_ascii=True, separators=(",", ":"))
    if not token or len(token) > 512:
        raise voice_actor_provider.VoiceLabProviderError("actor_identity_invalid")
    return token


def voice_actor_static_status() -> dict[str, Any]:
    try:
        package = voice_actor_provider.validate_actor_package(common.VOICE_ACTOR_ROOT)
        voice_actor_provider.inference_source_assets(common.GPT_SOVITS_SOURCE_ROOT)
        return {"ready": True, "actor_token": voice_actor_package_token(package), "blocker": ""}
    except Exception as exc:
        clear_voice_actor_runtime()
        return {"ready": False, "actor_token": "", "blocker": voice_actor_blocker(exc)}


def get_voice_actor_runtime(package: dict[str, Any] | None = None) -> dict[str, Any]:
    global VOICE_ACTOR_RUNTIME, VOICE_ACTOR_RUNTIME_FINGERPRINT
    package = package or voice_actor_provider.validate_actor_package(common.VOICE_ACTOR_ROOT)
    fingerprint = package["fingerprint"]
    if VOICE_ACTOR_RUNTIME is not None and VOICE_ACTOR_RUNTIME_FINGERPRINT == fingerprint:
        return VOICE_ACTOR_RUNTIME
    clear_voice_actor_runtime()
    runtime = voice_actor_provider.load_voice_actor_runtime(
        common.GPT_SOVITS_SOURCE_ROOT, common.VOICE_ACTOR_ROOT
    )
    if runtime.get("fingerprint") != fingerprint:
        raise voice_actor_provider.VoiceLabProviderError("actor_changed_during_load")
    latest = voice_actor_provider.validate_actor_package(common.VOICE_ACTOR_ROOT)
    if latest["fingerprint"] != fingerprint:
        raise voice_actor_provider.VoiceLabProviderError("actor_changed_during_load")
    VOICE_ACTOR_RUNTIME = runtime
    VOICE_ACTOR_RUNTIME_FINGERPRINT = fingerprint
    return runtime


def get_bound_voice_actor_runtime(expected_token: str) -> tuple[dict[str, Any], str]:
    expected_token = expected_token.strip()
    if expected_token and VOICE_ACTOR_RUNTIME is not None:
        runtime_token = voice_actor_package_token(
            {"fingerprint": VOICE_ACTOR_RUNTIME.get("fingerprint")}
        )
        if runtime_token == expected_token:
            return VOICE_ACTOR_RUNTIME, runtime_token

    package = voice_actor_provider.validate_actor_package(common.VOICE_ACTOR_ROOT)
    package_token = voice_actor_package_token(package)
    if expected_token and package_token != expected_token:
        clear_voice_actor_runtime()
        raise voice_actor_provider.VoiceLabProviderError("actor_changed_since_meeting_start")

    runtime = get_voice_actor_runtime(package)
    runtime_token = voice_actor_package_token({"fingerprint": runtime.get("fingerprint")})
    if expected_token and runtime_token != expected_token:
        clear_voice_actor_runtime()
        raise voice_actor_provider.VoiceLabProviderError("actor_changed_since_meeting_start")
    return runtime, runtime_token


def handle_voice_actor_preflight(_payload: dict[str, Any]) -> dict[str, Any]:
    started = common.now_ms()
    try:
        runtime = get_voice_actor_runtime()
        token = voice_actor_package_token({"fingerprint": runtime.get("fingerprint")})
        return {
            "ok": True,
            "stage": "voice_actor_preflight",
            "voice_id": "MyVoice",
            "language_code": "en",
            "device": str(runtime.get("device", "unknown")),
            "reference_cached": bool(runtime.get("reference_cached")),
            "actor_token": token,
            "elapsed_ms": common.now_ms() - started,
            "blocker": "",
        }
    except Exception as exc:
        return {
            "ok": False,
            "stage": "voice_actor_preflight",
            "voice_id": "MyVoice",
            "language_code": "en",
            "actor_token": "",
            "blocker": voice_actor_blocker(exc),
            "elapsed_ms": common.now_ms() - started,
        }


def handle_voice_actor_synthesize(payload: dict[str, Any]) -> dict[str, Any]:
    started = common.now_ms()
    if common.runtime_text_too_large(payload.get("text", ""), common.MAX_TTS_TEXT_CHARS):
        return {
            "ok": False,
            "stage": "voice_actor_synthesize",
            "blocker": "voice_actor:text_too_large",
            "max_chars": common.MAX_TTS_TEXT_CHARS,
        }
    text = common.compact_runtime_text(payload.get("text", ""), common.MAX_TTS_TEXT_CHARS)
    if not text:
        return {"ok": False, "stage": "voice_actor_synthesize", "blocker": "voice_actor:empty_text"}
    output_path: Path | None = None
    try:
        output_path = common.resolve_worker_path(
            payload.get("output_path", ""),
            common.CACHE_ROOT / "voice_actor_output.wav",
            common.ALLOWED_OUTPUT_ROOTS,
        )
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.unlink(missing_ok=True)
        expected = common.compact_runtime_text(payload.get("expected_actor_token", ""), 512)
        runtime, runtime_token = get_bound_voice_actor_runtime(expected)
        synthesis = voice_actor_provider.synthesize_voice_actor(runtime, text, output_path)
        if not output_path.is_file() or output_path.stat().st_size <= 44:
            raise voice_actor_provider.VoiceLabProviderError("inference_audio_invalid")
        return {
            "ok": True,
            "stage": "voice_actor_synthesize",
            "voice_id": "MyVoice",
            "language_code": "en",
            "device": synthesis["device"],
            "reference_cached": synthesis["reference_cached"],
            "sample_rate": synthesis["sample_rate"],
            "actor_token": runtime_token,
            "output_path": str(output_path),
            "elapsed_ms": common.now_ms() - started,
            "blocker": "",
        }
    except Exception as exc:
        clear_voice_actor_runtime()
        if output_path is not None:
            try:
                output_path.unlink(missing_ok=True)
            except Exception:
                pass
        return {
            "ok": False,
            "stage": "voice_actor_synthesize",
            "voice_id": "MyVoice",
            "language_code": "en",
            "blocker": voice_actor_blocker(exc),
            "elapsed_ms": common.now_ms() - started,
        }


def state_snapshot() -> dict[str, Any]:
    return {
        "asr_loaded": ASR_RUNTIME is not None,
        "asr_device": ASR_RUNTIME_DEVICE,
        "asr_compute_type": ASR_RUNTIME_COMPUTE,
        "asr_model_id": ASR_RUNTIME_MODEL_ID,
        "voice_actor_loaded": VOICE_ACTOR_RUNTIME is not None,
        "voice_actor_device": (
            str(VOICE_ACTOR_RUNTIME.get("device", "not_loaded"))
            if VOICE_ACTOR_RUNTIME
            else "not_loaded"
        ),
    }
