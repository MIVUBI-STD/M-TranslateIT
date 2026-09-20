from __future__ import annotations

import importlib.util
import types
from pathlib import Path

WORKER_PATH = Path(__file__).resolve().parents[1] / "realtime_local_worker.py"


def load_worker_module():
    spec = importlib.util.spec_from_file_location("translateit_worker_hotwords", WORKER_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_asr_passes_bounded_hotwords_to_faster_whisper(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    audio = tmp_path / "speech.wav"
    audio.write_bytes(b"RIFF" + b"0" * 64)
    captured = {}

    class FakeModel:
        def transcribe(self, _path, **kwargs):
            captured.update(kwargs)
            return [types.SimpleNamespace(text="MIVUBI hadir")], types.SimpleNamespace(
                language="id", language_probability=1.0
            )

    monkeypatch.setattr(worker.io_runtime.common, "resolve_worker_path", lambda *_args, **_kwargs: audio)
    monkeypatch.setattr(worker.io_runtime, "get_asr_runtime", lambda _payload=None: FakeModel())

    result = worker.handle_transcribe({
        "audio_path": str(audio),
        "language": "id",
        "hotwords": "  MIVUBI   mi vu bi   Vredeburg  ",
    })

    assert result["ok"] is True
    assert result["hotwords_applied"] is True
    assert captured["hotwords"] == "MIVUBI mi vu bi Vredeburg"


def test_asr_omits_empty_hotwords(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    audio = tmp_path / "speech.wav"
    audio.write_bytes(b"RIFF" + b"0" * 64)
    captured = {}

    class FakeModel:
        def transcribe(self, _path, **kwargs):
            captured.update(kwargs)
            return [types.SimpleNamespace(text="halo")], types.SimpleNamespace(
                language="id", language_probability=1.0
            )

    monkeypatch.setattr(worker.io_runtime.common, "resolve_worker_path", lambda *_args, **_kwargs: audio)
    monkeypatch.setattr(worker.io_runtime, "get_asr_runtime", lambda _payload=None: FakeModel())

    result = worker.handle_transcribe({"audio_path": str(audio), "hotwords": ""})

    assert result["ok"] is True
    assert result["hotwords_applied"] is False
    assert captured["hotwords"] is None
