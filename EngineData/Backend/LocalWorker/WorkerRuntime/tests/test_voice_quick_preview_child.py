from __future__ import annotations

import importlib.util
import wave
from pathlib import Path

import pytest

from test_voice_actor_inference import load_provider_module

PREVIEW_PATH = Path(__file__).resolve().parents[1] / "voice_lab_quick_preview.py"


def module():
    spec = importlib.util.spec_from_file_location("translateit_quick_preview", PREVIEW_PATH)
    assert spec is not None and spec.loader is not None
    loaded = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(loaded)
    return loaded


def setup_paths(tmp_path: Path):
    source = tmp_path / "source"
    source.mkdir()
    reference = tmp_path / "take.wav"
    reference.write_bytes(b"take")
    output = tmp_path / "Preview"
    output.mkdir()
    return source, reference, output


def test_preview_output_is_single_named_file_not_approved_actor(tmp_path: Path, monkeypatch):
    preview = module()
    source, reference, output = setup_paths(tmp_path)
    created = []

    def prepare(source_root, reference_wav, ref_text):
        assert source_root == source
        assert reference_wav == reference
        assert ref_text == "Good morning."
        return {"preview_only": True}

    def synthesize(runtime, text, output_path):
        assert runtime["preview_only"] is True
        assert text == preview.PREVIEW_TEXT
        with wave.open(str(output_path), "wb") as wav:
            wav.setnchannels(1)
            wav.setsampwidth(2)
            wav.setframerate(32000)
            wav.writeframes(b"\x01\x00" * 3200)
        created.append(output_path)
        return {"sample_rate": 32000}

    monkeypatch.setattr(preview, "prepare_pretrained_voice_preview", prepare)
    monkeypatch.setattr(preview, "synthesize_pretrained_voice_preview", synthesize)

    result = preview.preview_once(source, reference, output, "Good morning.")
    assert result == {
        "ok": True, "stage": "quick_voice_preview", "preview_only": True,
        "file": "quick_voice_preview.wav", "sample_rate": 32000,
    }
    assert created == [output / "quick_voice_preview.wav"]
    assert not (output / "actor.json").exists()
    with pytest.raises(preview.VoiceLabProviderError, match="preview_output_already_exists"):
        preview.preview_once(source, reference, output, "Good morning.")


def test_invalid_reference_or_output_rejected_without_model_load(tmp_path: Path, monkeypatch):
    preview = module()
    source, reference, output = setup_paths(tmp_path)
    monkeypatch.setattr(preview, "prepare_pretrained_voice_preview", lambda *_: pytest.fail("no model load"))
    with pytest.raises(preview.VoiceLabProviderError, match="preview_reference_text_invalid"):
        preview.preview_once(source, reference, output, "")
    with pytest.raises(preview.VoiceLabProviderError, match="preview_output_must_be_separate"):
        preview.preview_once(source, reference, tmp_path, "Hello")
    reference.unlink()
    with pytest.raises(preview.VoiceLabProviderError, match="preview_reference_missing"):
        preview.preview_once(source, reference, output, "Hello")


def test_failed_generation_leaves_no_partial_preview(tmp_path: Path, monkeypatch):
    preview = module()
    source, reference, output = setup_paths(tmp_path)
    monkeypatch.setattr(preview, "prepare_pretrained_voice_preview", lambda *_: {"preview_only": True})

    def fail(_runtime, _text, target):
        target.write_bytes(b"partial")
        raise RuntimeError("generation failure")

    monkeypatch.setattr(preview, "synthesize_pretrained_voice_preview", fail)
    with pytest.raises(RuntimeError, match="generation failure"):
        preview.preview_once(source, reference, output, "Hello")
    assert not (output / preview.OUTPUT_FILE).exists()


def test_invalid_generated_wav_is_removed(tmp_path: Path, monkeypatch):
    preview = module()
    source, reference, output = setup_paths(tmp_path)
    monkeypatch.setattr(preview, "prepare_pretrained_voice_preview", lambda *_: {"preview_only": True})

    def invalid(_runtime, _text, target):
        target.write_bytes(b"RIFF" + b"\0" * 100)
        return {"sample_rate": 32000}

    monkeypatch.setattr(preview, "synthesize_pretrained_voice_preview", invalid)
    with pytest.raises(preview.VoiceLabProviderError, match="preview_audio_invalid"):
        preview.preview_once(source, reference, output, "Hello")
    assert not (output / preview.OUTPUT_FILE).exists()

def test_preview_uses_training_reference_selector(tmp_path: Path, monkeypatch):
    preview = module()
    _, reference, _ = setup_paths(tmp_path)
    second = tmp_path / "take2.wav"
    second.write_bytes(b"second")
    candidates = [
        '{"line_id": 1, "exact_text": "One", "wav_path": "' + str(reference).replace("\\", "\\\\") + '"}',
        '{"line_id": 2, "exact_text": "Two", "wav_path": "' + str(second).replace("\\", "\\\\") + '"}',
    ]
    monkeypatch.setattr(preview, "wav_duration_ms", lambda _: 5000)
    observed = []

    def select(takes):
        observed.append(takes)
        return takes[1]

    monkeypatch.setattr(preview, "select_reference", select)
    path, exact_text = preview.choose_reference(candidates)
    assert (path, exact_text) == (second, "Two")
    assert [take["line_id"] for take in observed[0]] == [1, 2]


def test_preview_rejects_invalid_reference_candidate(tmp_path: Path):
    preview = module()
    with pytest.raises(preview.VoiceLabProviderError, match="preview_reference_candidate_invalid"):
        preview.choose_reference(['{"line_id": 1, "exact_text": "One", "wav_path": "missing.wav"}'])
