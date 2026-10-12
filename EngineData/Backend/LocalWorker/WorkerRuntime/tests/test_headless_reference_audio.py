from __future__ import annotations

import struct
import types
import wave

import nltk
import numpy as np

import voice_lab_upstream_stage as stage
from voice_lab_upstream_stage import _load_reference_audio


def test_reference_decoder_preserves_pcm_scale_channels_and_sample_rate(tmp_path):
    reference = tmp_path / "reference.wav"
    with wave.open(str(reference), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(32_000)
        writer.writeframes(struct.pack("<3h", -32768, 0, 16384))

    audio, sample_rate = _load_reference_audio(reference)
    assert sample_rate == 32_000
    assert tuple(audio.shape) == (1, 3)
    np.testing.assert_array_equal(audio.numpy(), [[-1.0, 0.0, 0.5]])


def test_headless_setup_uses_bundled_nltk_search_root(tmp_path, monkeypatch):
    (tmp_path / "GPT_SoVITS").mkdir()
    monkeypatch.setattr(nltk.data, "path", [])
    monkeypatch.setattr(stage.sys, "path", list(stage.sys.path))
    monkeypatch.setattr(stage, "_install_english_only_import_shims", lambda: None)
    monkeypatch.setattr(stage, "_install_english_only_tts_model_init", lambda: None)
    monkeypatch.setattr(stage.importlib, "import_module", lambda _name: types.SimpleNamespace())
    monkeypatch.setitem(stage.sys.modules, "tools.my_utils", types.ModuleType("tools.my_utils"))
    stage.install_headless_my_utils(tmp_path)
    stage.install_headless_my_utils(tmp_path)
    assert nltk.data.path == [str(tmp_path / "nltk_data")]
