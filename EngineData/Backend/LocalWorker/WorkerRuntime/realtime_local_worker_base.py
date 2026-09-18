from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import worker_io_runtime as io_runtime
import worker_runtime_common as common

translation_envelope = common.translation_envelope
RUNTIME_ROOT = common.RUNTIME_ROOT
USER_DATA_ROOT = common.USER_DATA_ROOT
RUNTIME_ASSETS_ROOT = common.RUNTIME_ASSETS_ROOT
ASR_MODEL_ROOT = common.ASR_MODEL_ROOT
TRANSLATION_MODEL_ROOT = common.TRANSLATION_MODEL_ROOT
TRANSLATION_MODEL = TRANSLATION_MODEL_ROOT / "_provider_not_installed"
GPT_SOVITS_SOURCE_ROOT = common.GPT_SOVITS_SOURCE_ROOT
VOICE_ACTOR_ROOT = common.VOICE_ACTOR_ROOT
CACHE_ROOT = common.CACHE_ROOT
LOG_ROOT = common.LOG_ROOT
ALLOWED_INPUT_ROOTS = common.ALLOWED_INPUT_ROOTS
ALLOWED_OUTPUT_ROOTS = common.ALLOWED_OUTPUT_ROOTS
MAX_WORKER_REQUEST_BYTES = common.MAX_WORKER_REQUEST_BYTES
MAX_TRANSLATION_TEXT_CHARS = common.MAX_TRANSLATION_TEXT_CHARS
MAX_TTS_TEXT_CHARS = common.MAX_TTS_TEXT_CHARS
MAX_TRANSCRIPT_TEXT_CHARS = common.MAX_TRANSCRIPT_TEXT_CHARS
MAX_AUDIO_INPUT_BYTES = common.MAX_AUDIO_INPUT_BYTES
MAX_REASONABLE_MODEL_TOKEN_LIMIT = common.MAX_REASONABLE_MODEL_TOKEN_LIMIT

TRANSLATION_RUNTIME: dict[str, dict[str, Any]] = {}
ASR_MODEL = io_runtime.ASR_MODEL
ASR_BACKUP_MODEL = io_runtime.ASR_BACKUP_MODEL

configured_absolute_root = common.configured_absolute_root
now_ms = common.now_ms
request_deadline_remaining_ms = common.request_deadline_remaining_ms
request_deadline_expired = common.request_deadline_expired
import_ready = common.import_ready
bounded_int = common.bounded_int
bounded_float = common.bounded_float
compact_runtime_text = common.compact_runtime_text
runtime_text_too_large = common.runtime_text_too_large
safe_command_name = common.safe_command_name
torch_status = common.torch_status
ctranslate2_status = common.ctranslate2_status
probe_gpu_runtime = common.probe_gpu_runtime
normalize_language = common.normalize_language
direction_pair = common.direction_pair
translation_runtime_config = common.translation_runtime_config
move_inputs_to_device = common.move_inputs_to_device
input_token_count = common.input_token_count
generation_eos_token_ids = common.generation_eos_token_ids
generation_pad_token_ids = common.generation_pad_token_ids
resolve_worker_path = common.resolve_worker_path

asr_model_ready = io_runtime.asr_model_ready
choose_asr_model = io_runtime.choose_asr_model
asr_runtime_config = io_runtime.asr_runtime_config
get_asr_runtime = io_runtime.get_asr_runtime
handle_asr_preload = io_runtime.handle_asr_preload
handle_transcribe = io_runtime.handle_transcribe
voice_actor_static_status = io_runtime.voice_actor_static_status
get_voice_actor_runtime = io_runtime.get_voice_actor_runtime
clear_voice_actor_runtime = io_runtime.clear_voice_actor_runtime
voice_actor_blocker = io_runtime.voice_actor_blocker
voice_actor_package_token = io_runtime.voice_actor_package_token
handle_voice_actor_preflight = io_runtime.handle_voice_actor_preflight
handle_voice_actor_synthesize = io_runtime.handle_voice_actor_synthesize


def translation_model_for_direction(
    source_language: str, target_language: str
) -> tuple[str, Path] | None:
    del source_language, target_language
    return None


def translation_model_ready(path: Path) -> bool:
    del path
    return False


def translation_input_token_limit(tokenizer: Any, model: Any) -> int | None:
    return translation_envelope.input_token_limit(
        tokenizer, model, MAX_REASONABLE_MODEL_TOKEN_LIMIT
    )


def get_translation_runtime(source_language: str, target_language: str) -> dict[str, Any]:
    del source_language, target_language
    raise RuntimeError("translation:provider_not_installed")


def handle_translation_preload(payload: dict[str, Any]) -> dict[str, Any]:
    del payload
    return {
        "ok": False,
        "stage": "translation_preload",
        "blocker": "translation:provider_not_installed",
    }


def handle_translate(payload: dict[str, Any]) -> dict[str, Any]:
    del payload
    return {"ok": False, "stage": "translate", "blocker": "translation:provider_not_installed"}


def status_action_items(blockers: list[str], warnings: list[str]) -> list[str]:
    actions: list[str] = []
    joined = ";".join(blockers + warnings)
    if "dependency:" in joined:
        actions.append("Install WorkerRuntime Python dependencies inside the worker environment.")
    if "faster_whisper" in joined:
        actions.append("Provide the approved local faster-whisper ASR runtime/model assets.")
    if "transformers" in joined or "torch" in joined or "translation_model_missing" in joined:
        actions.append("Repair the canonical local translation provider/model assets.")
    if "voice_actor:" in joined:
        actions.append(
            "Create and approve My Voice in VoiceLab, or repair the installed VoiceLab runtime assets."
        )
    if "cuda_unavailable" in joined:
        actions.append(
            "CUDA is optional; known unavailability uses explicit CPU degraded operation."
        )
    if "cuda:" in joined:
        actions.append(
            "Repair the locked CUDA runtime/probe failure; do not mask it with CPU fallback."
        )
    return list(dict.fromkeys(actions))


def build_status_payload(payload: dict[str, Any] | None = None) -> dict[str, Any]:
    faster_whisper_ready = import_ready("faster_whisper")
    transformers_ready = import_ready("transformers")
    gpu = probe_gpu_runtime(payload)
    torch_ready = bool(gpu["torch_import_ready"])
    torch_probe_ok = bool(gpu["torch_cuda_probe_ok"])
    ct2_ready = bool(gpu["ctranslate2_import_ready"])
    ct2_probe_ok = bool(gpu["ctranslate2_cuda_probe_ok"])
    known = bool(gpu["cuda_capability_known"])
    cpu_fallback = bool(gpu["cpu_fallback_active"])

    asr_primary_ready = asr_model_ready(ASR_MODEL)
    asr_backup_ready = asr_model_ready(ASR_BACKUP_MODEL)
    asr_active_ready = asr_primary_ready or asr_backup_ready
    asr_model_id, asr_model_path = choose_asr_model()
    asr_grade = (
        "primary" if asr_primary_ready else "fallback_degraded" if asr_backup_ready else "blocked"
    )
    translation_ready = translation_model_ready(TRANSLATION_MODEL)
    actor = voice_actor_static_status()
    voice_ready = bool(actor["ready"])

    blockers: list[str] = []
    warnings: list[str] = []
    if not faster_whisper_ready:
        blockers.append("dependency:faster_whisper_missing")
    if not transformers_ready:
        blockers.append("dependency:transformers_missing")
    if not torch_ready:
        blockers.append("dependency:torch_missing")
    if not ct2_ready:
        blockers.append("dependency:ctranslate2_missing")
    if torch_ready and not torch_probe_ok:
        blockers.append(str(gpu["torch_cuda_probe_blocker"]))
    if ct2_ready and not ct2_probe_ok:
        blockers.append(str(gpu["ctranslate2_cuda_probe_blocker"]))
    if not asr_active_ready:
        blockers.append("model:faster_whisper_large_v3_turbo_and_medium_missing")
    elif not asr_primary_ready:
        warnings.append("asr_primary_large_v3_turbo_missing_using_medium_fallback")
    if not asr_backup_ready:
        warnings.append("asr_backup_faster_whisper_medium_missing")
    if not translation_ready:
        blockers.append("model:translation_model_missing")
    if not voice_ready:
        blockers.append(str(actor["blocker"]))
    if cpu_fallback:
        warnings.append("cuda_unavailable_cpu_fallback_active")

    ready = (
        faster_whisper_ready
        and transformers_ready
        and torch_ready
        and ct2_ready
        and torch_probe_ok
        and ct2_probe_ok
        and asr_active_ready
        and translation_ready
        and voice_ready
    )
    note = (
        "Worker reports the required outbound AI capabilities available."
        if ready
        else "Worker is running, but one or more required outbound AI capabilities are unavailable."
    )
    if cpu_fallback:
        note += " CUDA capability is unavailable; CPU fallback is explicit degraded operation."
    elif not known and (torch_ready or ct2_ready):
        note += " CUDA capability probing failed; CPU fallback was not activated."

    state = io_runtime.state_snapshot()
    return {
        "ok": ready,
        "stage": "local_realtime_worker_preflight",
        "blocker": ";".join(blockers),
        "blockers": blockers,
        "warnings": warnings,
        "next_actions": status_action_items(blockers, warnings),
        "note": note,
        "provider_ready": ready,
        "readiness": {
            "asr": asr_active_ready and faster_whisper_ready,
            "translation_id_en": translation_ready and transformers_ready and torch_ready,
            "translation_en_id": translation_ready and transformers_ready and torch_ready,
            "translation_bidirectional": translation_ready and transformers_ready and torch_ready,
            "voice_actor_tts": voice_ready,
            "cuda_degraded": cpu_fallback,
        },
        "dependencies": {
            "faster_whisper": faster_whisper_ready,
            "transformers": transformers_ready,
            "torch": torch_ready,
            "ctranslate2": ct2_ready,
        },
        "models": {
            "asr_primary": {
                "id": "faster-whisper-large-v3-turbo",
                "ready": asr_primary_ready,
                "path": str(ASR_MODEL),
            },
            "asr_backup": {
                "id": "faster-whisper-medium",
                "ready": asr_backup_ready,
                "path": str(ASR_BACKUP_MODEL),
            },
            "translation_id_en": {
                "id": "translation-provider",
                "ready": translation_ready,
                "path": str(TRANSLATION_MODEL),
            },
            "translation_en_id": {
                "id": "translation-provider",
                "ready": translation_ready,
                "path": str(TRANSLATION_MODEL),
            },
        },
        "tts": {
            "ready": voice_ready,
            "provider": "gpt-sovits-v2proplus" if voice_ready else None,
            "voice_id": "MyVoice" if voice_ready else None,
            "language_code": "en",
            "blocker": str(actor["blocker"]),
            "actor_token": str(actor["actor_token"]),
        },
        "gpu": gpu,
        "asr_primary_model_ready": asr_primary_ready,
        "asr_model_ready": asr_active_ready,
        "asr_backup_model_ready": asr_backup_ready,
        "asr_active_model_id": asr_model_id,
        "asr_active_model_path": str(asr_model_path),
        "asr_readiness_grade": asr_grade,
        "translation_id_en_ready": translation_ready,
        "translation_en_id_ready": translation_ready,
        "translation_bidirectional_ready": translation_ready,
        "voice_actor_ready": voice_ready,
        "voice_actor_token": str(actor["actor_token"]),
        "faster_whisper_import_ready": faster_whisper_ready,
        "transformers_import_ready": transformers_ready,
        "torch_import_ready": torch_ready,
        "torch_cuda_available": bool(gpu["torch_cuda_available"]),
        "ctranslate2_cuda_available": bool(gpu["ctranslate2_cuda_available"]),
        "gpu_primary_requested": bool(gpu["cuda_primary_requested"]),
        "selected_device": str(gpu["selected_device"]),
        "selected_translation_device": str(gpu["selected_translation_device"]),
        "selected_compute_type": str(gpu["selected_compute_type"]),
        "fallback_reason": str(gpu["fallback_reason"]),
        "cuda_capability_known": known,
        "cpu_fallback_active": cpu_fallback,
        "cuda_probe_blocker": str(gpu["cuda_probe_blocker"]),
        "loaded": {
            "asr": state["asr_loaded"],
            "asr_device": state["asr_device"],
            "asr_compute_type": state["asr_compute_type"],
            "asr_model_id": state["asr_model_id"],
            "translation_directions": sorted(TRANSLATION_RUNTIME.keys()),
            "voice_actor": state["voice_actor_loaded"],
            "voice_actor_device": state["voice_actor_device"],
        },
    }


def failed_from_status(
    stage: str, status: dict[str, Any], extra: dict[str, Any] | None = None
) -> dict[str, Any]:
    result = {
        "ok": False,
        "stage": stage,
        "blocker": status.get("blocker", "runtime:not_ready"),
        "blockers": status.get("blockers", []),
        "warnings": status.get("warnings", []),
        "next_actions": status.get("next_actions", []),
        "note": status.get("note", "Runtime is not ready."),
        "selected_device": status.get("selected_device"),
        "selected_compute_type": status.get("selected_compute_type"),
        "fallback_reason": status.get("fallback_reason"),
    }
    if extra:
        result.update(extra)
    return result


def handle_status(payload: dict[str, Any]) -> dict[str, Any]:
    return build_status_payload(payload)


def handle_ping(_: dict[str, Any]) -> dict[str, Any]:
    return {"ok": True, "stage": "ping", "unix_ms": now_ms()}


def handle_standalone_text_translate(payload: dict[str, Any]) -> dict[str, Any]:
    started = now_ms()
    raw_source = translation_envelope.clean_source_text(payload.get("text", ""))
    if not raw_source:
        return {"ok": False, "stage": "translate", "blocker": "translation:empty_text"}
    if len(raw_source) > MAX_TRANSLATION_TEXT_CHARS:
        return {
            "ok": False,
            "stage": "translate",
            "blocker": "translation:text_too_large",
            "max_chars": MAX_TRANSLATION_TEXT_CHARS,
        }
    source = normalize_language(payload.get("source_language", "id"), "id")
    target = normalize_language(payload.get("target_language", "en"), "en")
    pair = direction_pair(source, target)
    selected = translation_model_for_direction(source, target)
    if selected is None:
        return handle_translate(payload)
    model_id, model_path = selected
    if not translation_model_ready(model_path):
        return handle_translate(payload)
    try:
        runtime = get_translation_runtime(source, target)
        limit = translation_input_token_limit(runtime["tokenizer"], runtime["model"])
        if limit is None:
            return {
                "ok": False,
                "stage": "translate",
                "model_id": model_id,
                "direction_pair": pair,
                "blocker": "translation:model_input_limit_unknown",
                "elapsed_ms": now_ms() - started,
            }
        plan = translation_envelope.standalone_plan(raw_source, runtime["tokenizer"], limit)
        if not plan:
            return {
                "ok": False,
                "stage": "translate",
                "model_id": model_id,
                "direction_pair": pair,
                "blocker": "translation:standalone_chunk_plan_empty",
                "elapsed_ms": now_ms() - started,
            }
        chunk_total = sum(len(paragraph) for paragraph in plan)
        outputs: list[list[str]] = []
        input_tokens = generated_tokens = max_budget = 0
        tokenization_ms = inference_ms = decode_ms = 0.0
        hit_ceiling = False
        last: dict[str, Any] | None = None
        chunk_index = 0
        for paragraph in plan:
            translated_paragraph: list[str] = []
            for chunk in paragraph:
                chunk_index += 1
                if request_deadline_expired(payload):
                    return {
                        "ok": False,
                        "stage": "translate",
                        "model_id": model_id,
                        "direction_pair": pair,
                        "chunk_index": chunk_index,
                        "chunk_count": chunk_total,
                        "blocker": "worker:request_deadline_expired",
                        "translated_text": "",
                    }
                request = dict(payload)
                request["text"] = chunk
                request["request_kind"] = "standalone_text_chunk"
                result = handle_translate(request)
                if not result.get("ok"):
                    failed = dict(result)
                    failed.update(
                        {
                            "translated_text": "",
                            "chunk_index": chunk_index,
                            "chunk_count": chunk_total,
                        }
                    )
                    return failed
                translated = str(result.get("translated_text", "")).strip()
                if not translated:
                    return {
                        "ok": False,
                        "stage": "translate",
                        "blocker": "translation:empty_output",
                        "translated_text": "",
                    }
                translated_paragraph.append(translated)
                input_tokens += int(result.get("input_tokens") or 0)
                generated_tokens += int(result.get("generated_tokens") or 0)
                tokenization_ms += float(result.get("tokenization_ms") or 0.0)
                inference_ms += float(result.get("inference_ms") or 0.0)
                decode_ms += float(result.get("decode_ms") or 0.0)
                max_budget = max(max_budget, int(result.get("generation_budget_tokens") or 0))
                hit_ceiling = hit_ceiling or bool(result.get("hit_token_ceiling"))
                last = result
            outputs.append(translated_paragraph)
        translated = translation_envelope.reassemble(outputs)
        if not translated or last is None:
            return {
                "ok": False,
                "stage": "translate",
                "blocker": "translation:empty_output",
                "translated_text": "",
            }
        combined = dict(last)
        combined.update(
            {
                "ok": True,
                "translated_text": translated,
                "input_tokens": input_tokens,
                "generated_tokens": generated_tokens,
                "generation_budget_tokens": max_budget,
                "hit_token_ceiling": hit_ceiling,
                "tokenization_ms": round(tokenization_ms, 2),
                "inference_ms": round(inference_ms, 2),
                "decode_ms": round(decode_ms, 2),
                "inference_tokens_per_second": (
                    round(generated_tokens / (inference_ms / 1000.0), 2)
                    if inference_ms > 0
                    else None
                ),
                "chunk_count": chunk_total,
                "paragraph_count": len(plan),
                "paragraph_structure_preserved": True,
                "complete": True,
                "finished_with_eos": True,
                "elapsed_ms": now_ms() - started,
                "blocker": "",
            }
        )
        return combined
    except translation_envelope.TranslationEnvelopeError as exc:
        return {
            "ok": False,
            "stage": "translate",
            "model_id": model_id,
            "direction_pair": pair,
            "blocker": str(exc),
            "translated_text": "",
            "elapsed_ms": now_ms() - started,
        }
    except Exception as exc:
        return {
            "ok": False,
            "stage": "translate",
            "model_id": model_id,
            "direction_pair": pair,
            "blocker": type(exc).__name__,
            "note": str(exc),
            "translated_text": "",
            "elapsed_ms": now_ms() - started,
        }


def handle_translate_request(payload: dict[str, Any]) -> dict[str, Any]:
    if str(payload.get("request_kind", "")).strip().lower() == "standalone_text":
        return handle_standalone_text_translate(payload)
    return handle_translate(payload)


handle_asr_preload = io_runtime.handle_asr_preload
handle_transcribe = io_runtime.handle_transcribe
handle_voice_actor_preflight = io_runtime.handle_voice_actor_preflight
handle_voice_actor_synthesize = io_runtime.handle_voice_actor_synthesize

HANDLERS = {
    "ping": handle_ping,
    "status": handle_status,
    "asr_preload": handle_asr_preload,
    "transcribe": handle_transcribe,
    "translation_preload": handle_translation_preload,
    "translate": handle_translate_request,
    "voice_actor_preflight": handle_voice_actor_preflight,
    "voice_actor_synthesize": handle_voice_actor_synthesize,
}


def respond(payload: dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(payload, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def main() -> int:
    for raw in sys.stdin:
        if len(raw.encode("utf-8", errors="ignore")) > MAX_WORKER_REQUEST_BYTES:
            respond(
                {
                    "ok": False,
                    "stage": "worker_request",
                    "blocker": "worker:request_too_large",
                    "max_bytes": MAX_WORKER_REQUEST_BYTES,
                }
            )
            continue
        try:
            request = json.loads(raw)
            if not isinstance(request, dict):
                respond(
                    {
                        "ok": False,
                        "stage": "worker_request",
                        "blocker": "worker:request_must_be_object",
                    }
                )
                continue
            command = safe_command_name(request.get("command", "status"))
            if request_deadline_expired(request):
                respond(
                    {
                        "ok": False,
                        "stage": command or "worker_request",
                        "blocker": "worker:request_deadline_expired",
                        "note": (
                            "The request reached the worker after its host deadline "
                            "and was not executed."
                        ),
                    }
                )
                continue
            handler = HANDLERS.get(command)
            if handler is None:
                respond(
                    {
                        "ok": False,
                        "stage": command or "unknown",
                        "blocker": "worker:unknown_command",
                    }
                )
                continue
            respond(handler(request))
        except Exception as exc:
            respond(
                {
                    "ok": False,
                    "stage": "worker_error",
                    "blocker": type(exc).__name__,
                    "note": str(exc),
                }
            )
    return 0
