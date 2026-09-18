from __future__ import annotations

import importlib.util
from pathlib import Path

WORKER_PATH = Path(__file__).resolve().parents[1] / "realtime_local_worker.py"


class CountingTokenizer:
    def __call__(self, text: str, **_kwargs):
        return {"input_ids": list(range(max(1, len(text.split()))))}


def load_worker_module():
    spec = importlib.util.spec_from_file_location(
        "translateit_translation_output_contract_worker", WORKER_PATH
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def success_result(text: str) -> dict:
    return {
        "ok": True,
        "stage": "translate",
        "translated_text": text,
        "complete": True,
        "finished_with_eos": True,
        "input_tokens": 4,
        "generated_tokens": 3,
        "generation_budget_tokens": 64,
        "hit_token_ceiling": False,
        "blocker": "",
    }


def install_fake_standalone_runtime(worker, monkeypatch) -> None:
    fake_model_path = Path("fake-translation-model")
    monkeypatch.setattr(
        worker.runtime,
        "translation_model_for_direction",
        lambda _source, _target: ("fake-model", fake_model_path),
    )
    monkeypatch.setattr(worker.runtime, "translation_model_ready", lambda _path: True)
    monkeypatch.setattr(
        worker.runtime,
        "get_translation_runtime",
        lambda _source, _target: {"tokenizer": CountingTokenizer(), "model": object()},
    )
    monkeypatch.setattr(
        worker.runtime,
        "translation_input_token_limit",
        lambda _tokenizer, _model: 1024,
    )


def set_original_translate(worker, monkeypatch, callback) -> None:
    monkeypatch.setattr(
        worker.runtime,
        worker._CONTEXT_POLICY_ORIGINAL,
        callback,
    )


def test_direct_translation_output_is_fail_closed_above_character_limit(monkeypatch) -> None:
    worker = load_worker_module()
    oversized = "x" * (worker.MAX_TRANSLATION_OUTPUT_CHARS + 1)
    set_original_translate(worker, monkeypatch, lambda _payload: success_result(oversized))

    result = worker.handle_translate(
        {"text": "halo", "source_language": "id", "target_language": "en"}
    )

    assert result["ok"] is False
    assert result["blocker"] == "translation:output_too_large"
    assert result["translated_text"] == ""
    assert result["max_chars"] == worker.MAX_TRANSLATION_OUTPUT_CHARS
    assert result["complete"] is False
    assert result["finished_with_eos"] is True


def test_direct_translation_accepts_output_at_character_limit(monkeypatch) -> None:
    worker = load_worker_module()
    exact = "x" * worker.MAX_TRANSLATION_OUTPUT_CHARS
    set_original_translate(worker, monkeypatch, lambda _payload: success_result(exact))

    result = worker.handle_translate(
        {"text": "halo", "source_language": "id", "target_language": "en"}
    )

    assert result["ok"] is True
    assert result["translated_text"] == exact
    assert result["complete"] is True
    assert result["finished_with_eos"] is True


def test_standalone_rejects_ok_chunk_without_complete_eos_and_discards_partial_output(
    monkeypatch,
) -> None:
    worker = load_worker_module()
    install_fake_standalone_runtime(worker, monkeypatch)
    calls = 0

    def fake_translate(_payload):
        nonlocal calls
        calls += 1
        if calls == 1:
            return success_result("first translated sentence")
        incomplete = success_result("second partial sentence")
        incomplete.update({"complete": False, "finished_with_eos": False})
        return incomplete

    set_original_translate(worker, monkeypatch, fake_translate)

    result = worker.runtime.HANDLERS["translate"](
        {
            "text": "First sentence. Second sentence.",
            "source_language": "id",
            "target_language": "en",
            "request_kind": "standalone_text",
        }
    )

    assert calls == 2
    assert result["ok"] is False
    assert result["blocker"] == "translation:output_incomplete"
    assert result["translated_text"] == ""
    assert result["chunk_index"] == 2
    assert result["chunk_count"] == 2
    assert result["complete"] is False
    assert result["finished_with_eos"] is False


def test_standalone_preserves_explicit_chunk_failure_and_discards_partial_output(
    monkeypatch,
) -> None:
    worker = load_worker_module()
    install_fake_standalone_runtime(worker, monkeypatch)
    calls = 0

    def fake_translate(_payload):
        nonlocal calls
        calls += 1
        if calls == 1:
            return success_result("first translated sentence")
        return {
            "ok": False,
            "stage": "translate",
            "blocker": "translation:output_ended_without_eos",
            "translated_text": "partial must not escape",
            "complete": False,
            "finished_with_eos": False,
        }

    set_original_translate(worker, monkeypatch, fake_translate)

    result = worker.runtime.HANDLERS["translate"](
        {
            "text": "First sentence. Second sentence.",
            "source_language": "id",
            "target_language": "en",
            "request_kind": "standalone_text",
        }
    )

    assert calls == 2
    assert result["ok"] is False
    assert result["blocker"] == "translation:output_ended_without_eos"
    assert result["translated_text"] == ""
    assert result["chunk_index"] == 2
    assert result["chunk_count"] == 2
    assert result["complete"] is False
    assert result["finished_with_eos"] is False


def test_standalone_reassembly_is_fail_closed_above_total_output_limit(monkeypatch) -> None:
    worker = load_worker_module()
    install_fake_standalone_runtime(worker, monkeypatch)
    per_chunk = (worker.MAX_TRANSLATION_OUTPUT_CHARS // 2) + 1
    calls = 0

    def fake_translate(_payload):
        nonlocal calls
        calls += 1
        return success_result(("a" if calls == 1 else "b") * per_chunk)

    set_original_translate(worker, monkeypatch, fake_translate)

    result = worker.runtime.HANDLERS["translate"](
        {
            "text": "First sentence. Second sentence.",
            "source_language": "id",
            "target_language": "en",
            "request_kind": "standalone_text",
        }
    )

    assert calls == 2
    assert result["ok"] is False
    assert result["blocker"] == "translation:output_too_large"
    assert result["translated_text"] == ""
    assert result["max_chars"] == worker.MAX_TRANSLATION_OUTPUT_CHARS
    assert result["chunk_count"] == 2
    assert result["complete"] is False
    assert result["finished_with_eos"] is True


def test_standalone_aggregates_chunk_translation_telemetry(monkeypatch) -> None:
    worker = load_worker_module()
    install_fake_standalone_runtime(worker, monkeypatch)
    calls = 0

    def fake_translate(_payload):
        nonlocal calls
        calls += 1
        result = success_result(f"translated {calls}")
        result.update(
            {
                "tokenization_ms": 1.25 * calls,
                "inference_ms": 10.0 * calls,
                "decode_ms": 0.5 * calls,
            }
        )
        return result

    set_original_translate(worker, monkeypatch, fake_translate)

    result = worker.runtime.HANDLERS["translate"](
        {
            "text": "First sentence. Second sentence.",
            "source_language": "id",
            "target_language": "en",
            "request_kind": "standalone_text",
        }
    )

    assert result["ok"] is True
    assert result["chunk_count"] == 2
    assert result["tokenization_ms"] == 3.75
    assert result["inference_ms"] == 30.0
    assert result["decode_ms"] == 1.5
    assert result["generated_tokens"] == 6
    assert result["inference_tokens_per_second"] == 200.0
