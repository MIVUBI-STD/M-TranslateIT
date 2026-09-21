from __future__ import annotations

import json
import wave
from pathlib import Path

import pytest
from test_worker_contract import load_worker_module


def load_provider_module():
    return load_worker_module().io_runtime.voice_actor_provider


def write_reference_wav(path: Path, duration_ms: int = 4_000) -> None:
    sample_rate = 32_000
    frames = sample_rate * duration_ms // 1_000
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(sample_rate)
        writer.writeframes(b"\0\0" * frames)


def write_actor(root: Path, revision: str | None = None) -> None:
    root.mkdir(parents=True, exist_ok=True)
    (root / "gpt.ckpt").write_bytes(b"gpt")
    (root / "sovits.pth").write_bytes(b"sovits")
    write_reference_wav(root / "reference.wav")
    (root / "actor.json").write_text(
        json.dumps(
            {
                "schema_version": 1,
                "engine": "gpt-sovits-v2proplus",
                "engine_revision": revision or "d523079fc05d9a8028d6085bffe4a2757c32abb6",
                "gpt_weight_file": "gpt.ckpt",
                "sovits_weight_file": "sovits.pth",
                "reference_wav_file": "reference.wav",
                "reference_text": "Tomorrow we will review the final project timeline.",
                "reference_duration_ms": 4_000,
                "held_out_evaluation_complete": True,
            }
        ),
        encoding="utf-8",
    )


def test_actor_package_validation_matches_approved_contract(tmp_path: Path) -> None:
    provider = load_provider_module()
    actor = tmp_path / "MyVoice"
    write_actor(actor)
    package = provider.validate_actor_package(actor)
    assert package["reference_duration_ms"] == 4_000
    assert package["gpt_path"] == actor / "gpt.ckpt"
    assert len(package["fingerprint"]) == 5
    write_actor(actor, revision="0" * 40)
    with pytest.raises(provider.VoiceLabProviderError, match="actor_engine_contract_mismatch"):
        provider.validate_actor_package(actor)


def test_source_working_directory_is_restored(tmp_path: Path) -> None:
    provider = load_provider_module()
    original = Path.cwd()
    target = tmp_path / "source"
    target.mkdir()
    with provider.source_working_directory(target):
        assert Path.cwd() == target
    assert Path.cwd() == original


def test_worker_reuses_actor_runtime_until_package_identity_changes(monkeypatch) -> None:
    worker = load_worker_module()
    worker.clear_voice_actor_runtime()
    current = {"fingerprint": (("actor.json", 1, 1),)}
    loads: list[object] = []
    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "validate_actor_package",
        lambda _path: {"fingerprint": current["fingerprint"]},
    )

    def load_runtime(_source, _actor):
        loads.append(current["fingerprint"])
        return {
            "fingerprint": current["fingerprint"],
            "device": "cpu",
            "reference_cached": True,
        }

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "load_voice_actor_runtime",
        load_runtime,
    )
    first = worker.get_voice_actor_runtime()
    second = worker.get_voice_actor_runtime()
    assert first is second
    assert len(loads) == 1
    current["fingerprint"] = (("actor.json", 2, 2),)
    third = worker.get_voice_actor_runtime()
    assert third is not first
    assert len(loads) == 2


def test_voice_actor_synthesis_uses_only_myvoice_path(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    cache = tmp_path / "CacheData"
    cache.mkdir()
    monkeypatch.setattr(worker.io_runtime.common, "CACHE_ROOT", cache)
    monkeypatch.setattr(worker.io_runtime.common, "ALLOWED_OUTPUT_ROOTS", [cache])
    fingerprint = (("actor.json", 1, 1),)
    validations: list[Path] = []

    def validate_actor(root: Path):
        validations.append(root)
        return {"fingerprint": fingerprint}

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "validate_actor_package",
        validate_actor,
    )
    monkeypatch.setattr(
        worker.io_runtime,
        "get_voice_actor_runtime",
        lambda _package=None: {
            "device": "cpu",
            "reference_cached": True,
            "fingerprint": fingerprint,
        },
    )

    def synthesize(_runtime, _text, output_path):
        output_path.write_bytes(b"R" * 80)
        return {"sample_rate": 32_000, "device": "cpu", "reference_cached": True}

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "synthesize_voice_actor",
        synthesize,
    )
    output = cache / "myvoice.wav"
    result = worker.handle_voice_actor_synthesize(
        {"text": "Hello from My Voice.", "output_path": str(output)}
    )
    assert result["ok"] is True
    assert result["voice_id"] == "MyVoice"
    assert result["sample_rate"] == 32_000
    assert Path(result["output_path"]) == output
    assert len(validations) == 1


def test_meeting_warm_actor_skips_redundant_disk_validation(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    cache = tmp_path / "CacheData"
    cache.mkdir()
    monkeypatch.setattr(worker.io_runtime.common, "CACHE_ROOT", cache)
    monkeypatch.setattr(worker.io_runtime.common, "ALLOWED_OUTPUT_ROOTS", [cache])
    fingerprint = (("actor.json", 7, 9),)
    runtime = {
        "device": "cpu",
        "reference_cached": True,
        "fingerprint": fingerprint,
    }
    monkeypatch.setattr(worker.io_runtime, "VOICE_ACTOR_RUNTIME", runtime)
    monkeypatch.setattr(worker.io_runtime, "VOICE_ACTOR_RUNTIME_FINGERPRINT", fingerprint)

    def unexpected_validation(_root):
        raise AssertionError("warm bound Meeting actor must not revalidate package from disk")

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "validate_actor_package",
        unexpected_validation,
    )

    def synthesize(received_runtime, _text, output_path):
        assert received_runtime is runtime
        output_path.write_bytes(b"R" * 80)
        return {"sample_rate": 32_000, "device": "cpu", "reference_cached": True}

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "synthesize_voice_actor",
        synthesize,
    )
    output = cache / "warm-bound.wav"
    expected = worker.io_runtime.voice_actor_package_token({"fingerprint": fingerprint})
    result = worker.handle_voice_actor_synthesize(
        {
            "text": "Warm Meeting voice.",
            "output_path": str(output),
            "expected_actor_token": expected,
        }
    )
    assert result["ok"] is True
    assert result["actor_token"] == expected
    assert Path(result["output_path"]) == output


def test_voice_actor_failure_removes_stale_output_and_never_falls_back(
    tmp_path: Path, monkeypatch
) -> None:
    worker = load_worker_module()
    worker.clear_voice_actor_runtime()
    cache = tmp_path / "CacheData"
    cache.mkdir()
    output = cache / "stale.wav"
    output.write_bytes(b"old" * 40)
    monkeypatch.setattr(worker.io_runtime.common, "CACHE_ROOT", cache)
    monkeypatch.setattr(worker.io_runtime.common, "ALLOWED_OUTPUT_ROOTS", [cache])

    def unavailable(_package=None):
        raise worker.io_runtime.voice_actor_provider.VoiceLabProviderError("approved_actor_missing")

    monkeypatch.setattr(worker.io_runtime, "get_voice_actor_runtime", unavailable)
    result = worker.handle_voice_actor_synthesize({"text": "Hello.", "output_path": str(output)})
    assert result["ok"] is False
    assert result["blocker"] == "voice_actor:approved_actor_missing"
    assert not output.exists()


def test_hard_voice_actor_synthesis_failure_clears_cached_runtime(
    tmp_path: Path, monkeypatch
) -> None:
    worker = load_worker_module()
    cache = tmp_path / "CacheData"
    cache.mkdir()
    monkeypatch.setattr(worker.io_runtime.common, "CACHE_ROOT", cache)
    monkeypatch.setattr(worker.io_runtime.common, "ALLOWED_OUTPUT_ROOTS", [cache])

    fingerprint = (("actor.json", 11, 12),)
    runtime = {
        "device": "cpu",
        "reference_cached": True,
        "fingerprint": fingerprint,
    }
    monkeypatch.setattr(worker.io_runtime, "VOICE_ACTOR_RUNTIME", runtime)
    monkeypatch.setattr(worker.io_runtime, "VOICE_ACTOR_RUNTIME_FINGERPRINT", fingerprint)

    def fail_synthesis(_runtime, _text, output_path):
        output_path.write_bytes(b"partial")
        raise RuntimeError("inference failed")

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "synthesize_voice_actor",
        fail_synthesis,
    )

    output = cache / "failed.wav"
    expected = worker.io_runtime.voice_actor_package_token({"fingerprint": fingerprint})
    result = worker.handle_voice_actor_synthesize(
        {
            "text": "Hard failure should invalidate the actor runtime.",
            "output_path": str(output),
            "expected_actor_token": expected,
        }
    )

    assert result["ok"] is False
    assert result["blocker"] == "voice_actor:runtime_failed:RuntimeError"
    assert worker.io_runtime.VOICE_ACTOR_RUNTIME is None
    assert worker.io_runtime.VOICE_ACTOR_RUNTIME_FINGERPRINT is None
    assert not output.exists()


def test_static_worker_readiness_requires_approved_actor_and_inference_assets(monkeypatch) -> None:
    worker = load_worker_module()
    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "validate_actor_package",
        lambda _root: {"fingerprint": (("actor.json", 1, 1),)},
    )
    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "inference_source_assets",
        lambda _root: {"gsv": Path("gsv")},
    )
    status = worker.voice_actor_static_status()
    assert status["ready"] is True
    assert status["actor_token"] == '[["actor.json",1,1]]'


def test_meeting_actor_token_rejects_mid_session_actor_change(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    worker.clear_voice_actor_runtime()
    cache = tmp_path / "CacheData"
    cache.mkdir()
    monkeypatch.setattr(worker.io_runtime.common, "CACHE_ROOT", cache)
    monkeypatch.setattr(worker.io_runtime.common, "ALLOWED_OUTPUT_ROOTS", [cache])
    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "validate_actor_package",
        lambda _root: {"fingerprint": (("actor.json", 2, 2),)},
    )
    output = cache / "voice.wav"
    result = worker.handle_voice_actor_synthesize(
        {
            "text": "Hello.",
            "output_path": str(output),
            "expected_actor_token": '[["actor.json",1,1]]',
        }
    )
    assert result["ok"] is False
    assert result["blocker"] == "voice_actor:actor_changed_since_meeting_start"
    assert not output.exists()


def test_worker_protocol_exposes_only_trained_actor_tts_commands() -> None:
    worker = load_worker_module()
    assert worker.HANDLERS["voice_actor_preflight"] is worker.handle_voice_actor_preflight
    assert worker.HANDLERS["voice_actor_synthesize"] is worker.handle_voice_actor_synthesize
    assert "tts_preflight" not in worker.HANDLERS
    assert "synthesize" not in worker.HANDLERS


def test_meeting_pace_factor_uses_actor_reference_rate_and_clamps() -> None:
    from voice_lab_gpt_sovits import meeting_pace_factor

    runtime = {
        "reference_text": "one two three four",
        "reference_duration_ms": 2000,
    }
    assert meeting_pace_factor(runtime, "one two three four", 2000) == 1.0
    assert meeting_pace_factor(runtime, "one two three four", 1000) == 1.10
    assert meeting_pace_factor(runtime, "one two three four", 4000) == 0.90
    assert meeting_pace_factor(runtime, "", 1000) == 1.0


def test_voice_actor_synthesize_forwards_native_speed_factor(tmp_path: Path, monkeypatch) -> None:
    worker = load_worker_module()
    cache = tmp_path / "CacheData"
    cache.mkdir()
    monkeypatch.setattr(worker.io_runtime.common, "CACHE_ROOT", cache)
    monkeypatch.setattr(worker.io_runtime.common, "ALLOWED_OUTPUT_ROOTS", [cache])

    runtime = {
        "device": "cpu",
        "reference_cached": True,
        "fingerprint": (("actor.json", 1, 1),),
        "reference_text": "one two three four",
        "reference_duration_ms": 2000,
    }
    monkeypatch.setattr(worker.io_runtime, "get_voice_actor_runtime", lambda _package=None: runtime)

    seen: list[float] = []

    def synthesize(_runtime, _text, output_path, *, speed_factor=1.0):
        seen.append(speed_factor)
        output_path.write_bytes(b"R" * 80)
        return {
            "sample_rate": 32_000,
            "device": "cpu",
            "reference_cached": True,
            "speed_factor": speed_factor,
        }

    monkeypatch.setattr(
        worker.io_runtime.voice_actor_provider,
        "synthesize_voice_actor",
        synthesize,
    )
    output = cache / "paced.wav"
    result = worker.handle_voice_actor_synthesize(
        {
            "text": "Translated output.",
            "source_text": "one two three four",
            "source_speech_duration_ms": 1000,
            "output_path": str(output),
        }
    )

    assert result["ok"] is True
    assert seen == [1.10]
    assert result["pace_applied"] is True
    assert result["pace_factor"] == 1.10
