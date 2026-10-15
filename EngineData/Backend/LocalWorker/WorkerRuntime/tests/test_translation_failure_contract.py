from __future__ import annotations

import importlib.util
from pathlib import Path

WORKER_PATH = Path(__file__).resolve().parents[1] / "realtime_local_worker.py"


def load_worker_module():
    spec = importlib.util.spec_from_file_location(
        "translateit_translation_failure_contract_worker", WORKER_PATH
    )
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def set_original_translate(worker, monkeypatch, callback) -> None:
    monkeypatch.setattr(
        worker.runtime,
        worker._TRANSLATION_CONTRACT_ORIGINAL,
        callback,
    )


def set_original_preload(worker, monkeypatch, callback) -> None:
    monkeypatch.setattr(
        worker.runtime,
        worker._PRELOAD_POLICY_ORIGINAL,
        callback,
    )


def runtime_cache() -> dict:
    return {
        "id->en": {"model": object(), "direction_pair": "id->en"},
        "en->id": {"model": object(), "direction_pair": "en->id"},
    }


def test_generic_provider_failure_is_stable_bounded_and_clears_shared_runtime(
    monkeypatch,
) -> None:
    worker = load_worker_module()
    cache = runtime_cache()
    monkeypatch.setattr(worker.runtime, "TRANSLATION_RUNTIME", cache)
    note = (" provider driver failure\n" * 200) + "\x00tail"

    set_original_translate(
        worker,
        monkeypatch,
        lambda _payload: {
            "ok": False,
            "stage": "translate",
            "blocker": "OSError",
            "note": note,
            "translated_text": "partial output must not escape",
        },
    )

    result = worker.handle_translate(
        {"text": "halo", "source_language": "id", "target_language": "en"}
    )

    assert result["ok"] is False
    assert result["blocker"] == "translation:provider_failed"
    assert result["translated_text"] == ""
    assert 0 < len(result["note"]) <= worker.MAX_PROTOCOL_NOTE_CHARS
    assert "\n" not in result["note"]
    assert "\x00" not in result["note"]
    assert cache == {}


def test_namespaced_provider_exception_is_preserved_and_clears_runtime(monkeypatch) -> None:
    worker = load_worker_module()
    cache = runtime_cache()
    monkeypatch.setattr(worker.runtime, "TRANSLATION_RUNTIME", cache)

    set_original_translate(
        worker,
        monkeypatch,
        lambda _payload: {
            "ok": False,
            "stage": "translate",
            "blocker": "RuntimeError",
            "note": "cuda:torch_probe_failed:RuntimeError",
        },
    )

    result = worker.handle_translate(
        {"text": "halo", "source_language": "id", "target_language": "en"}
    )

    assert result["ok"] is False
    assert result["blocker"] == "cuda:torch_probe_failed:RuntimeError"
    assert result["note"] == "cuda:torch_probe_failed:RuntimeError"
    assert cache == {}


def test_content_completion_failure_preserves_loaded_runtime_and_does_not_retry(
    monkeypatch,
) -> None:
    worker = load_worker_module()
    cache = runtime_cache()
    monkeypatch.setattr(worker.runtime, "TRANSLATION_RUNTIME", cache)
    calls = 0

    def fail_once(_payload):
        nonlocal calls
        calls += 1
        return {
            "ok": False,
            "stage": "translate",
            "blocker": "translation:output_ended_without_eos",
            "translated_text": "partial output",
            "complete": False,
            "finished_with_eos": False,
        }

    set_original_translate(worker, monkeypatch, fail_once)

    result = worker.handle_translate(
        {"text": "halo", "source_language": "id", "target_language": "en"}
    )

    assert calls == 1
    assert result["ok"] is False
    assert result["blocker"] == "translation:output_ended_without_eos"
    assert result["translated_text"] == ""
    assert set(cache) == {"id->en", "en->id"}


def test_preload_import_failure_uses_dependency_blocker_and_clears_runtime(monkeypatch) -> None:
    worker = load_worker_module()
    cache = runtime_cache()
    monkeypatch.setattr(worker.runtime, "TRANSLATION_RUNTIME", cache)

    set_original_preload(
        worker,
        monkeypatch,
        lambda _payload: {
            "ok": False,
            "stage": "translation_preload",
            "blocker": "ModuleNotFoundError",
            "note": (" transformers import failed " * 200) + "\x00tail",
        },
    )

    result = worker.handle_translation_preload({})

    assert result["ok"] is False
    assert result["blocker"] == "dependency:translation_provider_import_failed"
    assert 0 < len(result["note"]) <= worker.MAX_PROTOCOL_NOTE_CHARS
    assert "\x00" not in result["note"]
    assert cache == {}


def test_standalone_wrapper_normalizes_unscoped_failure_and_clears_runtime(monkeypatch) -> None:
    worker = load_worker_module()
    cache = runtime_cache()
    monkeypatch.setattr(worker.runtime, "TRANSLATION_RUNTIME", cache)
    monkeypatch.setattr(
        worker.runtime,
        "handle_translate_request",
        lambda _payload: {
            "ok": False,
            "stage": "translate",
            "blocker": "KeyError",
            "note": "standalone planner/provider state failed",
            "translated_text": "partial standalone output",
        },
    )

    result = worker.runtime.HANDLERS["translate"](
        {
            "text": "First sentence. Second sentence.",
            "source_language": "id",
            "target_language": "en",
            "request_kind": "standalone_text",
        }
    )

    assert result["ok"] is False
    assert result["blocker"] == "translation:provider_failed"
    assert result["translated_text"] == ""
    assert cache == {}
