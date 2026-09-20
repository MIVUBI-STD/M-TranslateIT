from __future__ import annotations

import re
import time
from pathlib import Path
from typing import Any

MODEL_ID = "milmmt-46-1b-v1.0"
HF_MODEL_ID = "xiaomi-research/MiLMMT-46-1B-v1.0"
MODEL_REVISION = "4fc480b6c58dec29c159dcdf9fde0f6d5c354995"
MODEL_DIRNAME = "xiaomi-research--MiLMMT-46-1B-v1.0"
REVISION_MARKER = ".translateit_model_revision"
INPUT_CONTEXT_LIMIT = 2048
STANDALONE_SOURCE_TOKEN_LIMIT = 1792
MAX_NEW_TOKENS = 256
MISSING_BLOCKER = "model:milmmt_46_1b_missing"
BASE_MISSING_BLOCKER = "model:translation_model_missing"

_HOST: dict[str, Any] | None = None
_ORIGINAL_BUILD_STATUS = None
_ORIGINAL_STATUS_ACTION_ITEMS = None


def _host() -> dict[str, Any]:
    if _HOST is None:
        raise RuntimeError("translation:milmmt_provider_not_installed")
    return _HOST


def language_name(code: str) -> str:
    names = {"id": "Indonesian", "en": "English"}
    if code not in names:
        raise ValueError("translation:direction_not_supported")
    return names[code]


def build_prompt(
    source_language: str,
    target_language: str,
    text: str,
    context_pairs: "list[tuple[str, str]]" = (),
    terminology: "list[tuple[str, str]]" = (),
    alternative_of: str = "",
) -> str:
    source_name = language_name(source_language)
    target_name = language_name(target_language)
    if alternative_of:
        lines = [
            f"Translate this from {source_name} to {target_name}.",
            "Produce one natural alternative wording that preserves exactly the same meaning and factual details.",
            "Do not explain, add facts, omit details, or copy the existing translation verbatim.",
            f"Existing translation: {alternative_of}",
        ]
    else:
        lines = [f"Translate this from {source_name} to {target_name}:"]
    if terminology:
        lines.append("Preferred terminology (use when the matching source term appears):")
        for term_source, term_target in terminology:
            lines.append(f"- {term_source} => {term_target}")
    for pair_source, pair_target in context_pairs:
        lines.append(f"{source_name}: {pair_source}")
        lines.append(f"{target_name}: {pair_target}")
    lines.append(f"{source_name}: {text}")
    lines.append(f"{target_name}:")
    return "\n".join(lines)


MAX_CONTEXT_PAIRS = 3
MAX_CONTEXT_PAIR_CHARS = 500
MAX_TERMINOLOGY_ENTRIES = 16
MAX_TERMINOLOGY_TERM_CHARS = 80


def source_term_occurs(source_text: str, term_source: str) -> bool:
    source_folded = source_text.casefold()
    term_folded = term_source.casefold()
    if not term_folded:
        return False
    pattern = rf"(?<!\w){re.escape(term_folded)}(?!\w)"
    return re.search(pattern, source_folded) is not None


def normalize_context_pairs(raw_pairs: object, host: dict) -> "list[tuple[str, str]]":
    if not isinstance(raw_pairs, list):
        return []
    pairs: "list[tuple[str, str]]" = []
    for raw_pair in raw_pairs[-MAX_CONTEXT_PAIRS:]:
        if not isinstance(raw_pair, (list, tuple)) or len(raw_pair) != 2:
            continue
        source = host["compact_runtime_text"](raw_pair[0], MAX_CONTEXT_PAIR_CHARS)
        target = host["compact_runtime_text"](raw_pair[1], MAX_CONTEXT_PAIR_CHARS)
        if source and target:
            pairs.append((source, target))
    return pairs


def normalize_terminology_entries(
    raw_entries: object,
    source_language: str,
    target_language: str,
    source_text: str,
    host: dict,
) -> "list[tuple[str, str]]":
    if not isinstance(raw_entries, list) or source_language == target_language:
        return []
    entries: "list[tuple[str, str]]" = []
    seen: set[tuple[str, str]] = set()
    for raw_entry in raw_entries:
        if not isinstance(raw_entry, dict):
            continue
        indonesian = host["compact_runtime_text"](
            raw_entry.get("indonesian", ""), MAX_TERMINOLOGY_TERM_CHARS
        )
        english = host["compact_runtime_text"](
            raw_entry.get("english", ""), MAX_TERMINOLOGY_TERM_CHARS
        )
        if not indonesian or not english:
            continue
        if source_language == "id" and target_language == "en":
            term_source, term_target = indonesian, english
        elif source_language == "en" and target_language == "id":
            term_source, term_target = english, indonesian
        else:
            continue
        if not source_term_occurs(source_text, term_source):
            continue
        source_key = term_source.casefold()
        target_key = term_target.casefold()
        if any(
            source_key == existing_source or target_key == existing_target
            for existing_source, existing_target in seen
        ):
            continue
        seen.add((source_key, target_key))
        entries.append((term_source, term_target))
        if len(entries) >= MAX_TERMINOLOGY_ENTRIES:
            break
    return entries


def translation_model_for_direction(source_language: str, target_language: str):
    host = _host()
    pair = host["direction_pair"](source_language, target_language)
    if pair in {"id->en", "en->id"}:
        return MODEL_ID, host["TRANSLATION_MODEL"]
    return None


def _has_model_weights(path: Path) -> bool:
    if (path / "model.safetensors").is_file():
        return True
    return (path / "model.safetensors.index.json").is_file() and any(
        path.glob("model-*.safetensors")
    )


def translation_model_ready(path: Path) -> bool:
    marker = path / REVISION_MARKER
    try:
        revision_ok = (
            marker.is_file() and marker.read_text(encoding="utf-8").strip() == MODEL_REVISION
        )
    except OSError:
        revision_ok = False
    tokenizer_ready = (path / "tokenizer.json").is_file() or (path / "tokenizer.model").is_file()
    return (
        path.is_dir()
        and (path / "config.json").is_file()
        and (path / "generation_config.json").is_file()
        and (path / "tokenizer_config.json").is_file()
        and tokenizer_ready
        and _has_model_weights(path)
        and revision_ok
    )


def translation_input_token_limit(_tokenizer: Any, _model: Any) -> int:
    return STANDALONE_SOURCE_TOKEN_LIMIT


def get_translation_runtime(
    source_language: str,
    target_language: str,
    *,
    assets_verified: bool = False,
) -> dict[str, Any]:
    host = _host()
    pair = host["direction_pair"](source_language, target_language)
    selected = translation_model_for_direction(source_language, target_language)
    if selected is None:
        raise ValueError("translation:direction_not_supported")
    model_id, model_path = selected
    runtimes = host["TRANSLATION_RUNTIME"]
    if pair in runtimes:
        return runtimes[pair]
    if runtimes:
        runtime = {**next(iter(runtimes.values())), "direction_pair": pair}
        runtimes[pair] = runtime
        return runtime
    if not assets_verified and not host["translation_model_ready"](model_path):
        raise RuntimeError(MISSING_BLOCKER)

    import torch
    import transformers
    from transformers import AutoModelForCausalLM, AutoTokenizer

    device, fallback_reason = host["translation_runtime_config"]()
    started = time.perf_counter()
    tokenizer = AutoTokenizer.from_pretrained(str(model_path), local_files_only=True)
    load_kwargs: dict[str, Any] = {"local_files_only": True}
    precision = "fp32"
    if device == "cuda":
        if not torch.cuda.is_bf16_supported():
            raise RuntimeError("translation:cuda_bf16_unavailable")
        load_kwargs.update({"device_map": {"": 0}, "torch_dtype": torch.bfloat16})
        precision = "bf16"
    model = AutoModelForCausalLM.from_pretrained(str(model_path), **load_kwargs)
    if device == "cuda":
        device_map = dict(getattr(model, "hf_device_map", {}) or {})
        if any(value not in {0, "cuda", "cuda:0"} for value in device_map.values()):
            raise RuntimeError("translation:model_offloaded_outside_cuda")
    else:
        model = model.to("cpu")
    model.eval()
    attention_backend = str(
        getattr(
            model.config,
            "_attn_implementation",
            getattr(model.config, "attn_implementation", ""),
        )
        or ""
    )
    runtime = {
        "direction_pair": pair,
        "model_id": model_id,
        "model_revision": MODEL_REVISION,
        "model_path": str(model_path),
        "tokenizer": tokenizer,
        "model": model,
        "device": device,
        "device_note": "cuda_available" if device == "cuda" else "cpu_runtime",
        "precision": precision,
        "translation_gpu_requested": True,
        "translation_torch_cuda_available": device == "cuda",
        "translation_degraded": device != "cuda",
        "translation_fallback_reason": fallback_reason,
        "torch_version": str(torch.__version__),
        "transformers_version": str(transformers.__version__),
        "attention_backend": attention_backend,
        "cold_load_ms": round((time.perf_counter() - started) * 1000.0, 2),
    }
    runtimes[pair] = runtime
    return runtime


def _continuation(
    generated: Any,
    prompt_tokens: int,
    tokenizer: Any,
    model: Any,
    budget: int,
) -> dict[str, Any]:
    host = _host()
    sequences = getattr(generated, "sequences", generated)
    try:
        values = sequences[0, prompt_tokens:]
    except Exception:
        values = sequences[0][prompt_tokens:]
    try:
        raw_ids = values.detach().cpu().tolist()
    except Exception:
        raw_ids = values.tolist() if hasattr(values, "tolist") else values
    if not isinstance(raw_ids, (list, tuple)):
        raise RuntimeError("translation:output_completion_unverifiable")
    ids = [int(value) for value in raw_ids]
    pad_ids = set(host["generation_pad_token_ids"](tokenizer, model))
    while ids and ids[-1] in pad_ids:
        ids.pop()
    if not ids:
        raise RuntimeError("translation:empty_generation_sequence")
    eos_ids = set(host["generation_eos_token_ids"](tokenizer, model))
    if not eos_ids:
        raise RuntimeError("translation:eos_token_unavailable")
    finished = ids[-1] in eos_ids
    hit_ceiling = len(ids) >= budget
    return {
        "ids": ids,
        "complete": finished,
        "finished_with_eos": finished,
        "generated_tokens": len(ids),
        "hit_token_ceiling": hit_ceiling,
        "blocker": ""
        if finished
        else (
            "translation:output_hit_token_ceiling_without_eos"
            if hit_ceiling
            else "translation:output_ended_without_eos"
        ),
    }


def _generation_budget(prompt_tokens: int, payload: dict[str, Any]) -> int:
    host = _host()
    requested = host["bounded_int"](
        payload.get("max_new_tokens", MAX_NEW_TOKENS),
        MAX_NEW_TOKENS,
        16,
        MAX_NEW_TOKENS,
    )
    authority = min(MAX_NEW_TOKENS, max(64, prompt_tokens * 2 + 32))
    return min(MAX_NEW_TOKENS, max(requested, authority))


def handle_translate(payload: dict[str, Any]) -> dict[str, Any]:
    host = _host()
    started = host["now_ms"]()
    if host["runtime_text_too_large"](payload.get("text", ""), host["MAX_TRANSLATION_TEXT_CHARS"]):
        return {
            "ok": False,
            "stage": "translate",
            "blocker": "translation:text_too_large",
            "max_chars": host["MAX_TRANSLATION_TEXT_CHARS"],
            "elapsed_ms": host["now_ms"]() - started,
        }
    text = host["compact_runtime_text"](payload.get("text", ""), host["MAX_TRANSLATION_TEXT_CHARS"])
    source_language = host["normalize_language"](payload.get("source_language", "id"), "id")
    target_language = host["normalize_language"](payload.get("target_language", "en"), "en")
    pair = host["direction_pair"](source_language, target_language)
    if not text:
        return {
            "ok": False,
            "stage": "translate",
            "blocker": "translation:empty_text",
            "direction_pair": pair,
            "elapsed_ms": host["now_ms"]() - started,
        }
    selected = translation_model_for_direction(source_language, target_language)
    if selected is None:
        return {
            "ok": False,
            "stage": "translate",
            "direction_pair": pair,
            "direction_supported": False,
            "blocker": "translation:direction_not_supported",
            "elapsed_ms": host["now_ms"]() - started,
        }
    model_id, model_path = selected
    runtimes = host["TRANSLATION_RUNTIME"]
    runtime_warm = bool(runtimes)
    model_asset_check_performed = not runtime_warm
    if model_asset_check_performed and not host["translation_model_ready"](model_path):
        return {
            "ok": False,
            "stage": "translate",
            "model_id": model_id,
            "model_revision": MODEL_REVISION,
            "model_path": str(model_path),
            "direction_pair": pair,
            "direction_supported": True,
            "blocker": MISSING_BLOCKER,
            "runtime_reused": False,
            "model_asset_check_performed": True,
            "elapsed_ms": host["now_ms"]() - started,
        }
    tokenization_ms: float | None = None
    inference_ms: float | None = None
    decode_ms: float | None = None
    inference_tokens_per_second: float | None = None
    try:
        runtime = get_translation_runtime(
            source_language,
            target_language,
            assets_verified=model_asset_check_performed,
        )
        tokenizer = runtime["tokenizer"]
        model = runtime["model"]
        context_pairs = normalize_context_pairs(payload.get("context_pairs"), host)
        terminology = normalize_terminology_entries(
            payload.get("terminology"),
            source_language,
            target_language,
            text,
            host,
        )
        alternative_of = host["compact_runtime_text"](
            payload.get("alternative_of", ""), host["MAX_TRANSLATION_TEXT_CHARS"]
        )
        prompt = build_prompt(
            source_language,
            target_language,
            text,
            context_pairs,
            terminology,
            alternative_of,
        )
        tokenization_started = time.perf_counter()
        inputs = tokenizer(
            prompt,
            add_special_tokens=False,
            return_tensors="pt",
            truncation=False,
        )
        tokenization_ms = round((time.perf_counter() - tokenization_started) * 1000.0, 2)
        prompt_tokens = host["input_token_count"](inputs)
        if prompt_tokens is None:
            raise RuntimeError("translation:input_token_count_unavailable")
        if prompt_tokens > INPUT_CONTEXT_LIMIT:
            return {
                "ok": False,
                "stage": "translate",
                "model_id": model_id,
                "model_revision": MODEL_REVISION,
                "direction_pair": pair,
                "blocker": "translation:input_too_long_for_model",
                "input_tokens": prompt_tokens,
                "prompt_tokens": prompt_tokens,
                "input_token_limit": INPUT_CONTEXT_LIMIT,
                "elapsed_ms": host["now_ms"]() - started,
            }
        inputs = host["move_inputs_to_device"](inputs, runtime["device"])
        generation_budget = _generation_budget(prompt_tokens, payload)
        import torch

        inference_started = time.perf_counter()
        with torch.inference_mode():
            generated = model.generate(
                **inputs,
                max_new_tokens=generation_budget,
                do_sample=False,
                use_cache=True,
            )
        inference_ms = round((time.perf_counter() - inference_started) * 1000.0, 2)
        completion = _continuation(generated, prompt_tokens, tokenizer, model, generation_budget)
        if inference_ms > 0:
            inference_tokens_per_second = round(
                completion["generated_tokens"] / (inference_ms / 1000.0),
                2,
            )
        if not completion["complete"]:
            return {
                "ok": False,
                "stage": "translate",
                "model_id": model_id,
                "model_revision": MODEL_REVISION,
                "direction_pair": pair,
                "blocker": completion["blocker"],
                "complete": False,
                "finished_with_eos": completion["finished_with_eos"],
                "generated_tokens": completion["generated_tokens"],
                "hit_token_ceiling": completion["hit_token_ceiling"],
                "tokenization_ms": tokenization_ms,
                "inference_ms": inference_ms,
                "decode_ms": decode_ms,
                "inference_tokens_per_second": inference_tokens_per_second,
                "elapsed_ms": host["now_ms"]() - started,
            }
        decode_started = time.perf_counter()
        translated = tokenizer.decode(completion["ids"], skip_special_tokens=True).strip()
        decode_ms = round((time.perf_counter() - decode_started) * 1000.0, 2)
        translated = host["translation_envelope"].compact_unit(translated)
        if not translated:
            raise RuntimeError("translation:empty_decoded_translation")
        return {
            "ok": True,
            "stage": "translate",
            "translation_contract": "canonical_bidirectional_id_en",
            "model_id": model_id,
            "model_revision": MODEL_REVISION,
            "model_path": str(model_path),
            "source_language": source_language,
            "target_language": target_language,
            "direction_pair": pair,
            "direction_supported": True,
            "device": runtime["device"],
            "device_note": runtime["device_note"],
            "precision": runtime["precision"],
            "translation_gpu_requested": runtime["translation_gpu_requested"],
            "translation_torch_cuda_available": runtime["translation_torch_cuda_available"],
            "translation_degraded": runtime["translation_degraded"],
            "translation_fallback_reason": runtime["translation_fallback_reason"],
            "runtime_reused": runtime_warm,
            "model_asset_check_performed": model_asset_check_performed,
            "context_pairs_used": len(context_pairs),
            "terminology_entries_used": len(terminology),
            "input_tokens": prompt_tokens,
            "prompt_tokens": prompt_tokens,
            "input_token_limit": INPUT_CONTEXT_LIMIT,
            "generation_budget_tokens": generation_budget,
            "complete": True,
            "finished_with_eos": True,
            "generated_tokens": completion["generated_tokens"],
            "hit_token_ceiling": completion["hit_token_ceiling"],
            "tokenization_ms": tokenization_ms,
            "inference_ms": inference_ms,
            "decode_ms": decode_ms,
            "inference_tokens_per_second": inference_tokens_per_second,
            "translated_text": translated,
            "elapsed_ms": host["now_ms"]() - started,
            "blocker": "",
        }
    except Exception as exc:
        return {
            "ok": False,
            "stage": "translate",
            "model_id": model_id,
            "model_revision": MODEL_REVISION,
            "direction_pair": pair,
            "blocker": type(exc).__name__,
            "note": str(exc),
            "tokenization_ms": tokenization_ms,
            "inference_ms": inference_ms,
            "decode_ms": decode_ms,
            "inference_tokens_per_second": inference_tokens_per_second,
            "elapsed_ms": host["now_ms"]() - started,
        }


def handle_translation_preload(payload: dict[str, Any]) -> dict[str, Any]:
    host = _host()
    started = host["now_ms"]()
    source_language = host["normalize_language"](payload.get("source_language", "id"), "id")
    target_language = host["normalize_language"](payload.get("target_language", "en"), "en")
    pair = host["direction_pair"](source_language, target_language)
    selected = translation_model_for_direction(source_language, target_language)
    if selected is None:
        return {
            "ok": False,
            "stage": "translation_preload",
            "direction_pair": pair,
            "blocker": "translation:direction_not_supported",
        }
    model_id, model_path = selected
    transformers_ready = host["import_ready"]("transformers")
    torch_ready = host["import_ready"]("torch")
    ready = host["translation_model_ready"](model_path)
    blockers: list[str] = []
    if not transformers_ready:
        blockers.append("dependency:transformers_missing")
    if not torch_ready:
        blockers.append("dependency:torch_missing")
    if not ready:
        blockers.append(MISSING_BLOCKER)
    if blockers:
        return {
            "ok": False,
            "stage": "translation_preload",
            "model_id": model_id,
            "model_revision": MODEL_REVISION,
            "model_path": str(model_path),
            "direction_pair": pair,
            "blocker": ";".join(blockers),
            "blockers": blockers,
            "elapsed_ms": host["now_ms"]() - started,
        }
    try:
        runtime = get_translation_runtime(
            source_language,
            target_language,
            assets_verified=True,
        )
        return {
            "ok": True,
            "stage": "translation_preload",
            "model_path": str(model_path),
            "model_id": model_id,
            "model_revision": MODEL_REVISION,
            "direction_pair": pair,
            "device": runtime["device"],
            "device_note": runtime["device_note"],
            "precision": runtime["precision"],
            "translation_gpu_requested": runtime["translation_gpu_requested"],
            "translation_torch_cuda_available": runtime["translation_torch_cuda_available"],
            "translation_degraded": runtime["translation_degraded"],
            "translation_fallback_reason": runtime["translation_fallback_reason"],
            "elapsed_ms": host["now_ms"]() - started,
            "warnings": (
                ["cuda_unavailable_cpu_fallback_active"] if runtime["translation_degraded"] else []
            ),
            "note": "Canonical MiLMMT bidirectional model loaded.",
        }
    except Exception as exc:
        return {
            "ok": False,
            "stage": "translation_preload",
            "model_id": model_id,
            "model_revision": MODEL_REVISION,
            "direction_pair": pair,
            "blocker": type(exc).__name__,
            "note": str(exc),
            "elapsed_ms": host["now_ms"]() - started,
        }


def status_action_items(blockers: list[str], warnings: list[str]) -> list[str]:
    clean_blockers = [value for value in blockers if value != MISSING_BLOCKER]
    actions = list(_ORIGINAL_STATUS_ACTION_ITEMS(clean_blockers, warnings))
    if MISSING_BLOCKER in blockers:
        actions.append(
            "Acquire the pinned MiLMMT-46-1B-v1.0 snapshot under RuntimeAssets/Translation/ModelData."
        )
    return list(dict.fromkeys(actions))


def build_status_payload(payload: dict[str, Any] | None = None) -> dict[str, Any]:
    result = dict(_ORIGINAL_BUILD_STATUS(payload))
    blockers = [
        MISSING_BLOCKER if value == BASE_MISSING_BLOCKER else value
        for value in result.get("blockers", [])
    ]
    result["blockers"] = blockers
    result["blocker"] = ";".join(blockers)
    result["next_actions"] = status_action_items(blockers, list(result.get("warnings", [])))
    models = dict(result.get("models", {}))
    for key in ("translation_id_en", "translation_en_id"):
        entry = dict(models.get(key, {}))
        entry.update(
            {
                "id": MODEL_ID,
                "revision": MODEL_REVISION,
                "path": str(_host()["TRANSLATION_MODEL"]),
            }
        )
        models[key] = entry
    result["models"] = models
    result["translation_model_id"] = MODEL_ID
    result["translation_model_revision"] = MODEL_REVISION
    return result


def install(namespace: dict[str, Any]) -> None:
    global _HOST, _ORIGINAL_BUILD_STATUS, _ORIGINAL_STATUS_ACTION_ITEMS
    _HOST = namespace
    _ORIGINAL_BUILD_STATUS = namespace["build_status_payload"]
    _ORIGINAL_STATUS_ACTION_ITEMS = namespace["status_action_items"]
    namespace["TRANSLATION_MODEL"] = namespace["TRANSLATION_MODEL_ROOT"] / MODEL_DIRNAME
    namespace["MILMMT_MODEL_ID"] = MODEL_ID
    namespace["MILMMT_HF_MODEL_ID"] = HF_MODEL_ID
    namespace["MILMMT_MODEL_REVISION"] = MODEL_REVISION
    namespace["translation_model_for_direction"] = translation_model_for_direction
    namespace["translation_model_ready"] = translation_model_ready
    namespace["translation_input_token_limit"] = translation_input_token_limit
    namespace["get_translation_runtime"] = get_translation_runtime
    namespace["handle_translate"] = handle_translate
    namespace["handle_translation_preload"] = handle_translation_preload
    namespace["status_action_items"] = status_action_items
    namespace["build_status_payload"] = build_status_payload
    namespace["HANDLERS"]["translation_preload"] = handle_translation_preload
