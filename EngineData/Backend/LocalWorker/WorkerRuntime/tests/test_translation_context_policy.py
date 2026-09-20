from __future__ import annotations

import importlib.util
import sys
import types
from pathlib import Path

import milmmt_translation_provider

WORKER_PATH = Path(__file__).resolve().parents[1] / "realtime_local_worker.py"


class _FakeInferenceMode:
    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return False


def load_worker_module():
    spec = importlib.util.spec_from_file_location("translateit_context_policy_worker", WORKER_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def install_fake_translation_runtime(worker) -> list[str]:
    prompts: list[str] = []

    class FakeTokenizer:
        eos_token_id = 2
        pad_token_id = 1

        def __call__(self, prompt, **kwargs):
            prompts.append(prompt)
            assert kwargs.get("add_special_tokens") is False
            assert kwargs.get("truncation") is False
            return {"input_ids": [[5, 6, 7]]}

        def decode(self, _ids, *, skip_special_tokens=True):
            assert skip_special_tokens is True
            return "translated output"

    class FakeModel:
        eos_token_id = 2
        pad_token_id = 1

        def generate(self, **_kwargs):
            return types.SimpleNamespace(sequences=[[5, 6, 7, 9, 2]])

    worker.TRANSLATION_RUNTIME.clear()
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
    return prompts


def test_only_authoritative_outbound_id_en_can_use_context(monkeypatch) -> None:
    worker = load_worker_module()
    monkeypatch.setattr(worker, "translation_model_ready", lambda _path: True)
    monkeypatch.setitem(
        sys.modules,
        "torch",
        types.SimpleNamespace(inference_mode=_FakeInferenceMode),
    )
    prompts = install_fake_translation_runtime(worker)

    context_pairs = [
        ["old-0-id", "old-0-en"],
        ["old-1-id", "old-1-en"],
        ["old-2-id", "old-2-en"],
        ["old-3-id", "old-3-en"],
    ]

    outbound = worker.handle_translate(
        {
            "text": "current outbound",
            "source_language": "id",
            "target_language": "en",
            "meeting_lane": "you",
            "meeting_session_id": "session-a",
            "meeting_generation": 7,
            "context_pairs": context_pairs,
        }
    )
    assert outbound["ok"] is True
    outbound_prompt = prompts[-1]
    assert "old-0-id" not in outbound_prompt
    for index in (1, 2, 3):
        assert f"old-{index}-id" in outbound_prompt
        assert f"old-{index}-en" in outbound_prompt
    assert "Indonesian: current outbound" in outbound_prompt

    incoming = worker.handle_translate(
        {
            "text": "current incoming",
            "source_language": "en",
            "target_language": "id",
            "meeting_lane": "incoming",
            "meeting_session_id": "session-a",
            "context_pairs": [["must-not-enter-en", "must-not-enter-id"]],
        }
    )
    assert incoming["ok"] is True
    incoming_prompt = prompts[-1]
    assert "must-not-enter-en" not in incoming_prompt
    assert "must-not-enter-id" not in incoming_prompt
    assert "English: current incoming" in incoming_prompt

    standalone = worker.handle_translate(
        {
            "text": "standalone",
            "source_language": "id",
            "target_language": "en",
            "context_pairs": [["must-not-enter-standalone", "ignored"]],
        }
    )
    assert standalone["ok"] is True
    assert "must-not-enter-standalone" not in prompts[-1]

    spoofed_lane = worker.handle_translate(
        {
            "text": "spoofed lane",
            "source_language": "id",
            "target_language": "en",
            "meeting_lane": "you",
            "context_pairs": [["must-not-enter-spoof", "ignored"]],
        }
    )
    assert spoofed_lane["ok"] is True
    assert "must-not-enter-spoof" not in prompts[-1]

    zero_generation = worker.handle_translate(
        {
            "text": "zero generation",
            "source_language": "id",
            "target_language": "en",
            "meeting_lane": "you",
            "meeting_session_id": "session-a",
            "meeting_generation": 0,
            "context_pairs": [["must-not-enter-zero", "ignored"]],
        }
    )
    assert zero_generation["ok"] is True
    assert "must-not-enter-zero" not in prompts[-1]

    wrong_direction = worker.handle_translate(
        {
            "text": "reverse on you lane",
            "source_language": "en",
            "target_language": "id",
            "meeting_lane": "you",
            "meeting_session_id": "session-a",
            "meeting_generation": 7,
            "context_pairs": [["must-not-enter-reverse", "ignored"]],
        }
    )
    assert wrong_direction["ok"] is True
    assert "must-not-enter-reverse" not in prompts[-1]


def test_terminology_is_relevant_and_directional(monkeypatch) -> None:
    worker = load_worker_module()
    monkeypatch.setattr(worker, "translation_model_ready", lambda _path: True)
    monkeypatch.setitem(
        sys.modules,
        "torch",
        types.SimpleNamespace(inference_mode=_FakeInferenceMode),
    )
    prompts = install_fake_translation_runtime(worker)

    terms = [
        {"indonesian": "pemugaran", "english": "restoration"},
        {"indonesian": "nama produk", "english": "product name"},
        {"indonesian": "tidak relevan", "english": "irrelevant"},
    ]
    result = worker.handle_translate(
        {
            "text": "Pemugaran nama produk dimulai besok.",
            "source_language": "id",
            "target_language": "en",
            "terminology": terms,
        }
    )
    assert result["ok"] is True
    prompt = prompts[-1]
    assert "- pemugaran => restoration" in prompt
    assert "- nama produk => product name" in prompt
    assert "tidak relevan" not in prompt

    reverse = worker.handle_translate(
        {
            "text": "The restoration starts tomorrow.",
            "source_language": "en",
            "target_language": "id",
            "terminology": terms,
        }
    )
    assert reverse["ok"] is True
    reverse_prompt = prompts[-1]
    assert "- restoration => pemugaran" in reverse_prompt
    assert "product name => nama produk" not in reverse_prompt


def test_alternative_prompt_preserves_source_and_existing_translation(monkeypatch) -> None:
    worker = load_worker_module()
    monkeypatch.setattr(worker, "translation_model_ready", lambda _path: True)
    monkeypatch.setitem(
        sys.modules,
        "torch",
        types.SimpleNamespace(inference_mode=_FakeInferenceMode),
    )
    prompts = install_fake_translation_runtime(worker)

    result = worker.handle_translate(
        {
            "text": "Saya belum sempat mengerjakannya.",
            "source_language": "id",
            "target_language": "en",
            "alternative_of": "I haven't had a chance to work on it yet.",
        }
    )
    assert result["ok"] is True
    prompt = prompts[-1]
    assert "Produce one natural alternative wording" in prompt
    assert "I haven't had a chance to work on it yet." in prompt
    assert "Indonesian: Saya belum sempat mengerjakannya." in prompt


def test_terminology_rejects_conflicting_source_or_target_mappings(monkeypatch) -> None:
    worker = load_worker_module()
    monkeypatch.setattr(worker, "translation_model_ready", lambda _path: True)
    monkeypatch.setitem(
        sys.modules,
        "torch",
        types.SimpleNamespace(inference_mode=_FakeInferenceMode),
    )
    prompts = install_fake_translation_runtime(worker)

    result = worker.handle_translate(
        {
            "text": "Pemugaran dan arsip dibahas hari ini.",
            "source_language": "id",
            "target_language": "en",
            "terminology": [
                {"indonesian": "pemugaran", "english": "restoration"},
                {"indonesian": "PEMUGARAN", "english": "renovation"},
                {"indonesian": "restorasi", "english": "restoration"},
                {"indonesian": "arsip", "english": "archive"},
            ],
        }
    )
    assert result["ok"] is True
    prompt = prompts[-1]
    assert "- pemugaran => restoration" in prompt
    assert "PEMUGARAN => renovation" not in prompt
    assert "restorasi => restoration" not in prompt
    assert "- arsip => archive" in prompt


def test_terminology_does_not_match_inside_larger_word() -> None:
    assert (
        milmmt_translation_provider.source_term_occurs(
            "Program API ini akan diperbarui besok.", "ram"
        )
        is False
    )
    assert (
        milmmt_translation_provider.source_term_occurs(
            "Program API ini akan diperbarui besok.", "API"
        )
        is True
    )
    assert (
        milmmt_translation_provider.source_term_occurs("Nama produk ini tetap sama.", "nama produk")
        is True
    )
    assert milmmt_translation_provider.source_term_occurs("APIx", "API") is False
