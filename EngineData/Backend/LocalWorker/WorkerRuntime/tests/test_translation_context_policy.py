from __future__ import annotations

import importlib.util
import sys
import types
from pathlib import Path

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


def _install_fake_worker(monkeypatch):
    worker = load_worker_module()
    monkeypatch.setattr(worker, "translation_model_ready", lambda _path: True)
    monkeypatch.setitem(
        sys.modules, "torch", types.SimpleNamespace(inference_mode=_FakeInferenceMode)
    )
    prompts = install_fake_translation_runtime(worker)
    return worker, prompts


def test_legacy_meeting_options_never_change_translation_prompt(monkeypatch) -> None:
    worker, prompts = _install_fake_worker(monkeypatch)
    request = {
        "text": "Coba lagi",
        "source_language": "id",
        "target_language": "en",
        "meeting_lane": "you",
        "meeting_session_id": "session-a",
        "meeting_generation": 4,
        "context_pairs": [["OLD_INSTRUCTION", "OLD_TRANSLATION"]],
        "translation_style": "formal",
        "terminology": [{"indonesian": "Coba lagi", "english": "UNTRUSTED_DICTIONARY"}],
        "alternative_of": "OLD_ALTERNATIVE",
    }
    result = worker.handle_translate(request)
    assert result["ok"] is True
    prompt = prompts[-1]
    assert "Indonesian: Coba lagi" in prompt
    for forbidden in [
        "OLD_INSTRUCTION", "OLD_TRANSLATION", "UNTRUSTED_DICTIONARY",
        "OLD_ALTERNATIVE", "formal, professional", "Preferred terminology",
    ]:
        assert forbidden not in prompt
    for retired in ("context_pairs_used", "terminology_entries_used", "translation_style"):
        assert retired not in result


def test_stored_preferences_are_inert_in_both_directions(monkeypatch) -> None:
    worker, prompts = _install_fake_worker(monkeypatch)
    cases = [
        ("id", "en", "Apakah kurang panjang?", "Indonesian:", "English:"),
        ("en", "id", "Hi please confirm this one", "English:", "Indonesian:"),
    ]
    for src, tgt, text, src_label, tgt_label in cases:
        response = worker.handle_translate({
            "text": text, "source_language": src, "target_language": tgt,
            "context_pairs": [["LEAK_SOURCE", "LEAK_TARGET"]],
            "terminology": [{"indonesian": "foo", "english": "BAR"}],
            "translation_style": "formal",
        })
        assert response["ok"] is True
        prompt = prompts[-1]
        assert f"{src_label} {text}" in prompt
        assert prompt.endswith(tgt_label)
        assert "LEAK_SOURCE" not in prompt
        assert "LEAK_TARGET" not in prompt
        assert "BAR" not in prompt


def test_incoming_prompt_uses_only_the_current_utterance(monkeypatch) -> None:
    worker, prompts = _install_fake_worker(monkeypatch)
    for i in range(25):
        response = worker.handle_translate({
            "text": f"Sentence {i}", "source_language": "en", "target_language": "id",
            "meeting_lane": "incoming", "context_pairs": [[f"old-{i}", "ignore"]],
        })
        assert response["ok"] is True
        assert prompts[-1].count(f"Sentence {i}") == 1
        assert f"old-{i}" not in prompts[-1]
