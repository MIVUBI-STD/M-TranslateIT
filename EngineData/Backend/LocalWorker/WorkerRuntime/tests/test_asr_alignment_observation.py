"""Source-only contracts for opt-in model-estimated ASR word timing evidence.

The test uses fake ASR words, not actual Whisper inference. These fixtures
cannot certify word alignment, translation quality, or live streaming.
"""
from __future__ import annotations

import importlib.util
import math
import types
import wave
from pathlib import Path

import pytest

WORKER_PATH = Path(__file__).resolve().parents[1] / "realtime_local_worker.py"


def load_worker_module():
    spec = importlib.util.spec_from_file_location("translateit_alignment_probe", WORKER_PATH)
    assert spec is not None and spec.loader is not None
    worker = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(worker)
    return worker


def write_probe_wav(path: Path, *, rate: int = 16_000, frames: int = 16_000) -> None:
    with wave.open(str(path), "wb") as audio:
        audio.setnchannels(1)
        audio.setsampwidth(2)
        audio.setframerate(rate)
        audio.writeframes(b"\x00\x00" * frames)


def model_word(text: str, start: float, end: float):
    return types.SimpleNamespace(word=text, start=start, end=end)


def make_runtime(tmp_path: Path, monkeypatch, *, words=None, transcript="Saya tidak setuju."):
    worker = load_worker_module()
    source_wav = tmp_path / "speech.wav"
    write_probe_wav(source_wav)
    observed = {}
    if words is None:
        words = [
            model_word(" Saya", 0.10, 0.25),
            model_word(" tidak", 0.26, 0.40),
            model_word(" setuju.", 0.42, 0.70),
        ]

    class FakeModel:
        def transcribe(self, _audio_path, **kwargs):
            observed.update(kwargs)
            return [
                types.SimpleNamespace(text=transcript, words=words)
            ], types.SimpleNamespace(language="id", language_probability=0.99)

    monkeypatch.setattr(
        worker.io_runtime.common, "resolve_worker_path",
        lambda *_args, **_kwargs: source_wav,
    )
    monkeypatch.setattr(worker.io_runtime, "get_asr_runtime", lambda _payload=None: FakeModel())
    return worker, observed, source_wav


def test_default_asr_path_does_not_request_or_expose_alignment(tmp_path, monkeypatch):
    worker, observed, source = make_runtime(tmp_path, monkeypatch)
    result = worker.handle_transcribe({"audio_path": str(source)})
    assert result["ok"] is True
    assert observed["word_timestamps"] is False
    assert observed["vad_filter"] is True
    assert "asr_alignment" not in result


def test_probe_returns_bounded_model_estimates_not_verified_alignment(tmp_path, monkeypatch):
    worker, observed, source = make_runtime(tmp_path, monkeypatch)
    result = worker.handle_transcribe({
        "audio_path": str(source), "alignment_probe": True, "vad_filter": False,
    })
    assert result["ok"] is True
    assert observed["word_timestamps"] is True
    assert observed["vad_filter"] is False
    evidence = result["asr_alignment"]
    assert evidence["evidence"] == "FASTER_WHISPER_ESTIMATED_WORD_TIMES"
    assert evidence["verified_semantic_boundary"] is False
    assert evidence["sample_rate_hz"] == 16_000
    assert evidence["asr_frame_count"] == 16_000
    assert len(evidence["words"]) == 3
    assert 1590 <= evidence["words"][0]["start_sample"] <= 1610
    assert evidence["words"][-1]["end_sample"] <= evidence["asr_frame_count"]


@pytest.mark.parametrize("flag", ["true", 1, None, [], {}])
def test_probe_flag_must_be_real_boolean(tmp_path, monkeypatch, flag):
    worker, observed, source = make_runtime(tmp_path, monkeypatch)
    result = worker.handle_transcribe({"audio_path": str(source), "alignment_probe": flag})
    assert result["ok"] is False
    assert result["blocker"] == "asr:alignment_probe_flag_invalid"
    assert observed == {}


def test_probe_rejects_worker_vad_before_inference(tmp_path, monkeypatch):
    worker, observed, source = make_runtime(tmp_path, monkeypatch)
    result = worker.handle_transcribe({"audio_path": str(source), "alignment_probe": True})
    assert result["ok"] is False
    assert result["blocker"] == "asr:alignment_requires_disabled_worker_vad"
    assert observed == {}


def test_probe_rejects_noncanonical_audio_before_inference(tmp_path, monkeypatch):
    worker, observed, source = make_runtime(tmp_path, monkeypatch)
    write_probe_wav(source, rate=48_000)
    result = worker.handle_transcribe({
        "audio_path": str(source), "alignment_probe": True, "vad_filter": False,
    })
    assert result["ok"] is False
    assert result["blocker"] == "asr:alignment_requires_pcm16_mono_16k_wav"
    assert observed == {}


def test_probe_rejects_truncated_wav_payload_before_inference(tmp_path, monkeypatch):
    worker, observed, source = make_runtime(tmp_path, monkeypatch)
    source.write_bytes(source.read_bytes()[:-10])
    result = worker.handle_transcribe({
        "audio_path": str(source), "alignment_probe": True, "vad_filter": False,
    })
    assert result["ok"] is False
    assert result["blocker"] == "asr:alignment_wav_data_truncated"
    assert observed == {}


@pytest.mark.parametrize("words,blocker", [
    ([model_word(" Saya", 0.2, 0.1)], "asr:alignment_word_invalid"),
    ([model_word(" Saya", -0.1, 0.1)], "asr:alignment_word_invalid"),
    ([model_word(" Saya", math.nan, 0.1)], "asr:alignment_word_invalid"),
    ([model_word(" Saya", 0.0, math.inf)], "asr:alignment_word_invalid"),
    ([model_word(" Saya", 0.8, 1.1)], "asr:alignment_word_range_invalid"),
    ([
        model_word(" Saya", 0.1, 0.4),
        model_word(" tidak", 0.3, 0.5),
    ], "asr:alignment_word_range_invalid"),
    ([
        model_word(" Saya", 0.1, 0.2),
        model_word(" bukan", 0.3, 0.4),
        model_word(" setuju.", 0.5, 0.6),
    ], "asr:alignment_transcript_mismatch"),
])
def test_invalid_or_inconsistent_alignment_fails_closed(
    tmp_path, monkeypatch, words, blocker
):
    worker, observed, source = make_runtime(tmp_path, monkeypatch, words=words)
    result = worker.handle_transcribe({
        "audio_path": str(source), "alignment_probe": True, "vad_filter": False,
    })
    assert observed["word_timestamps"] is True
    assert result["ok"] is False
    assert result["blocker"] == blocker
    assert result.get("asr_alignment") is None


def test_probe_rejects_too_many_model_words(tmp_path, monkeypatch):
    words = [
        model_word(" a", index / 1_000, (index + 0.4) / 1_000)
        for index in range(513)
    ]
    worker, observed, source = make_runtime(tmp_path, monkeypatch, words=words)
    result = worker.handle_transcribe({
        "audio_path": str(source), "alignment_probe": True, "vad_filter": False,
    })
    assert observed["word_timestamps"] is True
    assert result["ok"] is False
    assert result["blocker"] == "asr:alignment_word_limit_exceeded"
