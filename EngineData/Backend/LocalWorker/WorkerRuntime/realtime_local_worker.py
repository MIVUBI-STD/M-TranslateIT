from __future__ import annotations

import json
import math
import sys

import milmmt_translation_provider
import realtime_local_worker_base as runtime

_PROVIDER_SENTINEL = "_translateit_milmmt_provider_installed"
_CONTEXT_POLICY_SENTINEL = "_translateit_translation_context_policy_installed"
_CONTEXT_POLICY_ORIGINAL = "_translateit_translation_context_policy_original_handle_translate"
_PRELOAD_POLICY_SENTINEL = "_translateit_translation_preload_policy_installed"
_PRELOAD_POLICY_ORIGINAL = (
    "_translateit_translation_preload_policy_original_handle_translation_preload"
)

MAX_PROTOCOL_STAGE_CHARS = 96
MAX_PROTOCOL_BLOCKER_CHARS = 512
MAX_PROTOCOL_NOTE_CHARS = 1_000
MAX_TRANSLATION_OUTPUT_CHARS = runtime.MAX_TRANSLATION_TEXT_CHARS * 2

_TRANSLATION_DOMAIN_PREFIXES = (
    "translation:",
    "model:",
    "dependency:",
    "cuda:",
    "worker:",
)
_TRANSLATION_NON_INVALIDATING_BLOCKERS = {
    "translation:empty_text",
    "translation:text_too_large",
    "translation:direction_not_supported",
    "translation:input_too_long_for_model",
    "translation:output_hit_token_ceiling_without_eos",
    "translation:output_ended_without_eos",
    "translation:output_incomplete",
    "translation:output_too_large",
    "translation:empty_output",
    "translation:standalone_chunk_plan_empty",
    "translation:chunk_count_limit_exceeded",
    "worker:request_deadline_expired",
}


class _NonFiniteJsonNumber(ValueError):
    pass


if not getattr(runtime, _PROVIDER_SENTINEL, False):
    milmmt_translation_provider.install(vars(runtime))
    setattr(runtime, _PROVIDER_SENTINEL, True)


def _single_translation_domain_blocker(value: str) -> bool:
    return (
        bool(value)
        and value.startswith(_TRANSLATION_DOMAIN_PREFIXES)
        and all(char.isascii() and (char.isalnum() or char in "_:.-") for char in value)
    )


def _safe_translation_domain_blocker(value) -> str:
    text = runtime.compact_runtime_text(value, MAX_PROTOCOL_BLOCKER_CHARS)
    if not text:
        return ""
    parts = text.split(";")
    if parts and all(_single_translation_domain_blocker(part) for part in parts):
        return text
    return ""


def _translation_failure_blocker(result: dict, fallback: str) -> str:
    raw_blocker = runtime.compact_runtime_text(
        result.get("blocker", ""), MAX_PROTOCOL_BLOCKER_CHARS
    )
    blocker = _safe_translation_domain_blocker(raw_blocker)
    if blocker:
        return blocker

    note = runtime.compact_runtime_text(result.get("note", ""), MAX_PROTOCOL_NOTE_CHARS)
    blocker = _safe_translation_domain_blocker(note)
    if blocker:
        return blocker

    if raw_blocker in {"ImportError", "ModuleNotFoundError"}:
        return "dependency:translation_provider_import_failed"
    if raw_blocker == "MemoryError":
        return "translation:provider_memory_exhausted"
    return fallback


def _translation_failure_invalidates_runtime(blocker: str) -> bool:
    return blocker not in _TRANSLATION_NON_INVALIDATING_BLOCKERS


def _translation_failure_contract(result, fallback: str):
    if not isinstance(result, dict) or result.get("ok"):
        return result

    failed = dict(result)
    blocker = _translation_failure_blocker(failed, fallback)
    failed["blocker"] = blocker
    if "translated_text" in failed:
        failed["translated_text"] = ""
    if "note" in failed:
        note = runtime.compact_runtime_text(failed.get("note", ""), MAX_PROTOCOL_NOTE_CHARS)
        if note:
            failed["note"] = note
        else:
            failed.pop("note", None)
    if _translation_failure_invalidates_runtime(blocker):
        runtime.TRANSLATION_RUNTIME.clear()
    return failed


def _translation_contract_result(result):
    if not isinstance(result, dict):
        return result
    if not result.get("ok"):
        return _translation_failure_contract(result, "translation:provider_failed")

    translated = str(result.get("translated_text", ""))
    if not translated.strip():
        failed = dict(result)
        failed.update(
            {
                "ok": False,
                "blocker": "translation:empty_output",
                "translated_text": "",
                "complete": False,
            }
        )
        return _translation_failure_contract(failed, "translation:provider_failed")

    if result.get("complete") is not True or result.get("finished_with_eos") is not True:
        failed = dict(result)
        failed.update(
            {
                "ok": False,
                "blocker": "translation:output_incomplete",
                "translated_text": "",
                "complete": False,
                "finished_with_eos": bool(result.get("finished_with_eos")),
            }
        )
        return _translation_failure_contract(failed, "translation:provider_failed")

    if len(translated) > MAX_TRANSLATION_OUTPUT_CHARS:
        failed = dict(result)
        failed.update(
            {
                "ok": False,
                "blocker": "translation:output_too_large",
                "translated_text": "",
                "max_chars": MAX_TRANSLATION_OUTPUT_CHARS,
                "complete": False,
            }
        )
        return _translation_failure_contract(failed, "translation:provider_failed")

    return result


def _translation_context_authorized(payload: dict) -> bool:
    lane = str(payload.get("meeting_lane", "")).strip().lower()
    source = runtime.normalize_language(payload.get("source_language", "id"), "id")
    target = runtime.normalize_language(payload.get("target_language", "en"), "en")
    session_id = str(payload.get("meeting_session_id", "")).strip()
    generation = payload.get("meeting_generation")
    if isinstance(generation, bool):
        return False
    try:
        generation = int(generation)
    except (TypeError, ValueError):
        return False
    return (
        lane == "you" and source == "id" and target == "en" and bool(session_id) and generation > 0
    )


def _translation_payload_with_context_policy(payload):
    if not isinstance(payload, dict) or "context_pairs" not in payload:
        return payload
    if _translation_context_authorized(payload):
        return payload
    sanitized = dict(payload)
    sanitized.pop("context_pairs", None)
    return sanitized


def _context_guarded_runtime_translate(payload):
    original = getattr(runtime, _CONTEXT_POLICY_ORIGINAL)
    result = original(_translation_payload_with_context_policy(payload))
    return _translation_contract_result(result)


if not getattr(runtime, _CONTEXT_POLICY_SENTINEL, False):
    setattr(runtime, _CONTEXT_POLICY_ORIGINAL, runtime.handle_translate)
    runtime.handle_translate = _context_guarded_runtime_translate
    setattr(runtime, _CONTEXT_POLICY_SENTINEL, True)


def _contract_guarded_runtime_preload(payload):
    original = getattr(runtime, _PRELOAD_POLICY_ORIGINAL)
    result = original(payload)
    return _translation_failure_contract(result, "translation:runtime_load_failed")


if not getattr(runtime, _PRELOAD_POLICY_SENTINEL, False):
    setattr(runtime, _PRELOAD_POLICY_ORIGINAL, runtime.handle_translation_preload)
    runtime.handle_translation_preload = _contract_guarded_runtime_preload
    setattr(runtime, _PRELOAD_POLICY_SENTINEL, True)


def _contract_guarded_translate_request(payload):
    return _translation_contract_result(runtime.handle_translate_request(payload))


runtime.HANDLERS["translation_preload"] = runtime.handle_translation_preload
runtime.HANDLERS["translate"] = _contract_guarded_translate_request

# The old exec-based entrypoint exposed one mutable module namespace. Preserve
# only the bounded test/diagnostic injection surface while implementation truth
# now lives explicitly in `runtime`.
_RUNTIME_OVERRIDE_NAMES = (
    "asr_model_ready",
    "import_ready",
    "probe_gpu_runtime",
    "translation_model_ready",
    "translation_runtime_config",
    "voice_actor_static_status",
)


def _call_with_runtime_overrides(callback, *args):
    local = globals()
    previous: dict[str, object] = {}
    applied: list[str] = []
    for name in _RUNTIME_OVERRIDE_NAMES:
        if name not in local:
            continue
        previous[name] = getattr(runtime, name)
        setattr(runtime, name, local[name])
        applied.append(name)
    try:
        return callback(*args)
    finally:
        for name in reversed(applied):
            setattr(runtime, name, previous[name])


def handle_translate(payload):
    return _call_with_runtime_overrides(runtime.handle_translate, payload)


def handle_translation_preload(payload):
    return _call_with_runtime_overrides(runtime.handle_translation_preload, payload)


def build_status_payload(payload=None):
    return _call_with_runtime_overrides(runtime.build_status_payload, payload)


def get_translation_runtime(source_language: str, target_language: str):
    return _call_with_runtime_overrides(
        runtime.get_translation_runtime,
        source_language,
        target_language,
    )


def _reject_non_finite_json_constant(_value: str):
    raise _NonFiniteJsonNumber


def _contains_non_finite_json_number(value) -> bool:
    if isinstance(value, float):
        return not math.isfinite(value)
    if isinstance(value, dict):
        return any(_contains_non_finite_json_number(item) for item in value.values())
    if isinstance(value, list):
        return any(_contains_non_finite_json_number(item) for item in value)
    return False


def _parse_protocol_request(raw: str):
    try:
        request = json.loads(raw, parse_constant=_reject_non_finite_json_constant)
    except _NonFiniteJsonNumber:
        return None, "worker:non_finite_json_number"
    except json.JSONDecodeError:
        return None, "worker:invalid_json"
    if _contains_non_finite_json_number(request):
        return None, "worker:non_finite_json_number"
    if not isinstance(request, dict):
        return None, "worker:request_must_be_object"
    return request, ""


def _bounded_protocol_response(payload):
    if not isinstance(payload, dict):
        return {
            "ok": False,
            "stage": "worker_error",
            "blocker": "worker:response_must_be_object",
        }
    result = dict(payload)
    if "stage" in result:
        result["stage"] = runtime.compact_runtime_text(result["stage"], MAX_PROTOCOL_STAGE_CHARS)
    if "blocker" in result:
        result["blocker"] = runtime.compact_runtime_text(
            result["blocker"], MAX_PROTOCOL_BLOCKER_CHARS
        )
    if "note" in result:
        result["note"] = runtime.compact_runtime_text(result["note"], MAX_PROTOCOL_NOTE_CHARS)
    return result


def _respond_protocol(payload, request_id: str | None = None) -> None:
    response = _bounded_protocol_response(payload)
    # Optional for standalone probes; required for Rust-managed requests.
    # Echo only the host-supplied identity, overriding a handler's field.
    if request_id is not None:
        response["request_id"] = request_id
    sys.stdout.write(json.dumps(response, ensure_ascii=False, allow_nan=False) + "\n")
    sys.stdout.flush()


def _handler_failure(exc: Exception) -> dict:
    exception_type = "".join(
        char for char in type(exc).__name__ if char.isascii() and (char.isalnum() or char == "_")
    )[:96]
    return {
        "ok": False,
        "stage": "worker_error",
        "blocker": f"worker:handler_failed:{exception_type or 'Exception'}",
        "note": str(exc),
    }


def __getattr__(name: str):
    return getattr(runtime, name)


def __dir__() -> list[str]:
    return sorted(set(globals()) | set(dir(runtime)))


def main() -> int:
    for raw in sys.stdin:
        if len(raw.encode("utf-8", errors="ignore")) > runtime.MAX_WORKER_REQUEST_BYTES:
            _respond_protocol(
                {
                    "ok": False,
                    "stage": "worker_request",
                    "blocker": "worker:request_too_large",
                    "max_bytes": runtime.MAX_WORKER_REQUEST_BYTES,
                }
            )
            continue

        request, framing_blocker = _parse_protocol_request(raw)
        if framing_blocker:
            _respond_protocol(
                {
                    "ok": False,
                    "stage": "worker_request",
                    "blocker": framing_blocker,
                }
            )
            continue
        assert request is not None
        request_id = request.get("request_id")
        if request_id is not None and (
            not isinstance(request_id, str)
            or not request_id
            or len(request_id) > 128
            or not request_id.isascii()
            or not all(char.isalnum() or char in "-_" for char in request_id)
        ):
            _respond_protocol({
                "ok": False,
                "stage": "worker_request",
                "blocker": "worker:invalid_request_id",
            })
            continue

        command = runtime.safe_command_name(request.get("command", "status"))
        if runtime.request_deadline_expired(request):
            _respond_protocol(
                {
                    "ok": False,
                    "stage": command or "worker_request",
                    "blocker": "worker:request_deadline_expired",
                    "note": (
                        "The request reached the worker after its host deadline "
                        "and was not executed."
                    ),
                },
                request_id,
            )
            continue
        handler = runtime.HANDLERS.get(command)
        if handler is None:
            _respond_protocol(
                {
                    "ok": False,
                    "stage": command or "unknown",
                    "blocker": "worker:unknown_command",
                },
                request_id,
            )
            continue
        try:
            _respond_protocol(handler(request), request_id)
        except Exception as exc:
            _respond_protocol(_handler_failure(exc), request_id)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
