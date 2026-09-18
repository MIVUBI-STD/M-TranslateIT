from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import types
from pathlib import Path

import pytest

import milmmt_translation_provider

WORKER_PATH = Path(__file__).resolve().parents[1] / "realtime_local_worker.py"


def load_worker_module():
    spec = importlib.util.spec_from_file_location("translateit_realtime_local_worker", WORKER_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class _FakeInferenceMode:
    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False


def _install_fake_generate_runtime(worker, sequences) -> dict:
    captured: dict = {}

    class FakeTokenizer:
        eos_token_id = 2
        pad_token_id = 1

        def __call__(self, prompt, **kwargs):
            del prompt
            assert kwargs.get("add_special_tokens") is False
            assert kwargs.get("truncation") is False
            return {"input_ids": [[5, 6, 7]]}

        def decode(self, ids, *, skip_special_tokens=True):
            assert skip_special_tokens is True
            return "halo dunia"

    class FakeModel:
        eos_token_id = 2
        pad_token_id = 1

        def generate(self, **kwargs):
            captured.update(kwargs)
            return [list(sequence) for sequence in sequences]

    worker.TRANSLATION_RUNTIME["id->en"] = {
        "tokenizer": FakeTokenizer(),
        "model": FakeModel(),
        "device": "cpu",
        "device_note": "cpu_runtime",
        "precision": "fp32",
        "translation_gpu_requested": True,
        "translation_torch_cuda_available": False,
        "translation_degraded": True,
        "translation_fallback_reason": "torch_cuda_unavailable",
    }
    return captured



def test_voice_provider_keeps_heavy_upstream_stage_off_worker_import_path() -> None:
    provider_path = WORKER_PATH.parent / "voice_lab_gpt_sovits.py"
    source = provider_path.read_text(encoding="utf-8")

    assert (
        "from voice_lab_upstream_stage import install_headless_my_utils\n"
        not in source.split("def create_tts_runtime", 1)[0]
    )
    create_runtime = source.split("def create_tts_runtime", 1)[1]
    assert "from voice_lab_upstream_stage import install_headless_my_utils" in create_runtime


def test_translate_routes_to_milmmt_without_legacy_mode_output(monkeypatch) -> None:
    worker = load_worker_module()
    monkeypatch.setattr(worker, "translation_model_ready", lambda _path: False)
    result = worker.handle_translate(
        {"text": "halo", "source_language": "id", "target_language": "en"}
    )
    assert result["ok"] is False
    assert "mode" not in result
    assert result["direction_pair"] == "id->en"
    assert result["blocker"] == "model:milmmt_46_1b_missing"
    assert result["model_revision"] == worker.MILMMT_MODEL_REVISION


def test_reverse_direction_uses_same_canonical_milmmt_model(monkeypatch) -> None:
    worker = load_worker_module()
    monkeypatch.setattr(worker, "translation_model_ready", lambda _path: False)
    result = worker.handle_translate(
        {"text": "hello", "source_language": "en", "target_language": "id"}
    )
    assert result["ok"] is False
    assert result["direction_pair"] == "en->id"
    assert result["blocker"] == "model:milmmt_46_1b_missing"
    assert worker.translation_model_for_direction("id", "en") == (
        "milmmt-46-1b-v1.0",
        worker.TRANSLATION_MODEL,
    )
    assert worker.translation_model_for_direction("en", "id") == (
        "milmmt-46-1b-v1.0",
        worker.TRANSLATION_MODEL,
    )


def test_worker_status_reports_milmmt_revision_for_both_directions(monkeypatch) -> None:
    worker = load_worker_module()
    monkeypatch.setattr(worker, "import_ready", lambda _name: True)
    monkeypatch.setattr(worker, "asr_model_ready", lambda _path: True)
    monkeypatch.setattr(worker, "translation_model_ready", lambda _path: True)
    monkeypatch.setattr(
        worker,
        "probe_gpu_runtime",
        lambda _payload=None: {
            "torch_import_ready": True,
            "torch_cuda_probe_ok": True,
            "torch_cuda_available": True,
            "torch_cuda_probe_blocker": "",
            "ctranslate2_import_ready": True,
            "ctranslate2_cuda_probe_ok": True,
            "ctranslate2_cuda_available": True,
            "ctranslate2_cuda_probe_blocker": "",
            "cuda_capability_known": True,
            "cpu_fallback_active": False,
            "cuda_probe_blocker": "",
            "cuda_primary_requested": True,
            "selected_device": "cuda",
            "selected_translation_device": "cuda",
            "selected_compute_type": "int8_float16",
            "fallback_reason": "",
        },
    )
    monkeypatch.setattr(
        worker,
        "voice_actor_static_status",
        lambda: {"ready": True, "actor_token": "actor-v1", "blocker": ""},
    )
    status = worker.build_status_payload({})
    assert status["ok"] is True
    for key in ("translation_id_en", "translation_en_id"):
        assert status["models"][key]["id"] == "milmmt-46-1b-v1.0"
        assert status["models"][key]["revision"] == worker.MILMMT_MODEL_REVISION
        assert status["models"][key]["path"] == str(worker.TRANSLATION_MODEL)


def test_milmmt_generate_options_are_greedy_deterministic_and_have_no_forced_bos(
    monkeypatch,
) -> None:
    worker = load_worker_module()
    fake_torch = types.SimpleNamespace(inference_mode=_FakeInferenceMode)
    monkeypatch.setitem(sys.modules, "torch", fake_torch)
    monkeypatch.setattr(worker, "translation_model_ready", lambda _path: True)
    captured = _install_fake_generate_runtime(worker, [[1, 2, 3, 9, 9, 2]])

    result = worker.handle_translate(
        {"text": "halo", "source_language": "id", "target_language": "en"}
    )

    assert result["ok"] is True
    assert result["translated_text"] == "halo dunia"
    assert captured["max_new_tokens"] == milmmt_translation_provider.MAX_NEW_TOKENS
    assert captured["do_sample"] is False
    assert captured["use_cache"] is True
    assert "return_dict_in_generate" not in captured
    assert "forced_bos_token_id" not in captured


def test_milmmt_standalone_source_limit_reserves_prompt_headroom() -> None:
    worker = load_worker_module()
    assert worker.translation_input_token_limit(object(), object()) == 1792


def test_milmmt_continuation_rejects_token_ceiling_without_eos(monkeypatch) -> None:
    worker = load_worker_module()
    fake_torch = types.SimpleNamespace(inference_mode=_FakeInferenceMode)
    monkeypatch.setitem(sys.modules, "torch", fake_torch)
    monkeypatch.setattr(worker, "translation_model_ready", lambda _path: True)
    _install_fake_generate_runtime(worker, [[1, 2, 3] + [9] * 70])

    result = worker.handle_translate(
        {
            "text": "halo",
            "source_language": "id",
            "target_language": "en",
            "max_new_tokens": 16,
        }
    )

    assert result["ok"] is False
    assert result["complete"] is False
    assert result["hit_token_ceiling"] is True
    assert result["blocker"] == "translation:output_hit_token_ceiling_without_eos"


def test_legacy_userdata_label_cannot_escape_allowed_root(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    user_root = tmp_path / "app-local-data"
    cache_root = user_root / "CacheData"
    monkeypatch.setattr(worker, "USER_DATA_ROOT", user_root)
    with pytest.raises(ValueError, match="worker:path_outside_allowed_roots"):
        worker.resolve_worker_path(
            "UserData/CacheData/../LogData/not-allowed.wav",
            cache_root / "default.wav",
            [cache_root],
        )


def test_configured_runtime_root_must_be_absolute(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    monkeypatch.setenv("TRANSLATEIT_TEST_ROOT", "relative/path")
    with pytest.raises(RuntimeError, match="translateit_test_root_must_be_absolute"):
        worker.configured_absolute_root("TRANSLATEIT_TEST_ROOT", tmp_path)


def test_asr_failure_contract_preserves_owned_security_blocker(monkeypatch) -> None:
    worker = load_worker_module()

    def reject_path(*_args, **_kwargs):
        raise ValueError("worker:path_outside_allowed_roots")

    monkeypatch.setattr(worker.io_runtime.common, "resolve_worker_path", reject_path)
    result = worker.handle_transcribe({"audio_path": "outside.wav"})

    assert result["ok"] is False
    assert result["blocker"] == "worker:path_outside_allowed_roots"


def test_asr_runtime_failure_is_namespaced(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    audio = tmp_path / "segment.wav"
    audio.write_bytes(b"RIFF" + b"0" * 64)

    monkeypatch.setattr(
        worker.io_runtime.common,
        "resolve_worker_path",
        lambda *_args, **_kwargs: audio,
    )

    def fail_runtime(_payload=None):
        raise RuntimeError("unexpected inference failure")

    monkeypatch.setattr(worker.io_runtime, "get_asr_runtime", fail_runtime)
    result = worker.handle_transcribe({"audio_path": str(audio)})

    assert result["ok"] is False
    assert result["blocker"] == "asr:runtime_failed:RuntimeError"


def test_newline_json_protocol_rejects_unknown_command() -> None:
    completed = subprocess.run(
        [sys.executable, str(WORKER_PATH)],
        input='{"command":"not_a_real_command"}\n',
        text=True,
        capture_output=True,
        timeout=5,
        check=False,
    )
    assert completed.returncode == 0
    payload = json.loads(completed.stdout.strip())
    assert payload["ok"] is False
    assert payload["blocker"] == "worker:unknown_command"


def test_newline_protocol_rejects_already_expired_request() -> None:
    completed = subprocess.run(
        [sys.executable, str(WORKER_PATH)],
        input='{"command":"ping","deadline_unix_ms":1,"deadline_ms":5000}\n',
        text=True,
        capture_output=True,
        timeout=5,
        check=False,
    )
    assert completed.returncode == 0
    payload = json.loads(completed.stdout.strip())
    assert payload["ok"] is False
    assert payload["blocker"] == "worker:request_deadline_expired"


def test_gpu_probe_uses_cpu_only_for_known_unavailable_capability(monkeypatch) -> None:
    worker = load_worker_module()
    common = worker.io_runtime.common
    monkeypatch.setattr(
        common,
        "torch_status",
        lambda: {
            "import_ready": True,
            "cuda_probe_ok": True,
            "cuda_available": False,
            "blocker": "",
        },
    )
    monkeypatch.setattr(
        common,
        "ctranslate2_status",
        lambda: {
            "import_ready": True,
            "cuda_probe_ok": True,
            "cuda_available": False,
            "blocker": "",
        },
    )
    gpu = worker.probe_gpu_runtime({})
    assert gpu["cuda_capability_known"] is True
    assert gpu["cpu_fallback_active"] is True
    assert gpu["selected_device"] == "cpu"
    assert gpu["selected_translation_device"] == "cpu"
    assert gpu["fallback_reason"] == "cuda_unavailable"


def test_build_prompt_embeds_rolling_context_pairs_in_official_format() -> None:
    from milmmt_translation_provider import build_prompt

    prompt = build_prompt(
        "id",
        "en",
        "Besok dia memimpin rapat.",
        [("Itu kakak saya.", "That is my older brother.")],
    )
    assert prompt == (
        "Translate this from Indonesian to English:\n"
        "Indonesian: Itu kakak saya.\n"
        "English: That is my older brother.\n"
        "Indonesian: Besok dia memimpin rapat.\n"
        "English:"
    )
    assert prompt.count("Translate this from") == 1


def test_normalize_context_pairs_caps_at_three_and_sanitizes() -> None:
    from milmmt_translation_provider import MAX_CONTEXT_PAIRS, normalize_context_pairs

    host = {"compact_runtime_text": lambda value, limit: str(value or "")[:limit]}
    raw = [[f"s{i}", f"t{i}"] for i in range(5)]
    pairs = normalize_context_pairs(raw, host)
    assert len(pairs) == MAX_CONTEXT_PAIRS
    assert pairs[0] == ("s2", "t2")
    assert pairs[-1] == ("s4", "t4")
    assert normalize_context_pairs("not-a-list", host) == []
    assert normalize_context_pairs([[1, 2, 3], ["a", None]], host) == []
