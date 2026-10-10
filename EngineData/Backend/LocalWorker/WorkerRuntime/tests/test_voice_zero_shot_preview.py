from __future__ import annotations

from pathlib import Path

import pytest

from test_voice_actor_inference import load_provider_module


def test_pretrained_preview_resolves_only_pinned_v2proplus_weights(tmp_path: Path, monkeypatch) -> None:
    provider = load_provider_module()
    source = tmp_path / "source"
    gsv = source / "GPT_SoVITS"
    (gsv / "pretrained_models" / "v2Pro").mkdir(parents=True)
    (gsv / "pretrained_models" / "s1v3.ckpt").write_bytes(b"gpt")
    (gsv / "pretrained_models" / "v2Pro" / "s2Gv2ProPlus.pth").write_bytes(b"sovits")
    monkeypatch.setattr(provider, "inference_source_assets", lambda _source: {"gsv": gsv})
    assets = provider.pretrained_voice_preview_assets(source)
    assert assets["pretrained_gpt"].name == "s1v3.ckpt"
    assert assets["pretrained_sovits"].name == "s2Gv2ProPlus.pth"
    assert "actor_dir" not in assets


def test_pretrained_preview_is_separate_from_approved_actor(tmp_path: Path, monkeypatch) -> None:
    provider = load_provider_module()
    reference = tmp_path / "reference.wav"
    reference.write_bytes(b"reference")
    captured = []

    monkeypatch.setattr(provider, "require_regular_file", lambda *_args: (100, 1))
    monkeypatch.setattr(provider, "wav_signal_metrics_pcm16", lambda _path: {
        "duration_ms": 5000.0,
        "silence_fraction": 0.10,
        "clipping_fraction": 0.0,
        "active_rms": 3000.0,
        "dc_offset": 0.0,
    })
    monkeypatch.setattr(provider, "pretrained_voice_preview_assets", lambda _source: {
        "pretrained_gpt": tmp_path / "s1v3.ckpt",
        "pretrained_sovits": tmp_path / "s2Gv2ProPlus.pth",
    })

    def fake_runtime(_source, _assets, gpt, sovits, wav):
        captured.append((gpt.name, sovits.name, wav))
        return {"tts": object(), "reference_wav": wav}

    monkeypatch.setattr(provider, "create_tts_runtime", fake_runtime)
    runtime = provider.prepare_pretrained_voice_preview(tmp_path, reference, "Hello there")
    assert captured == [("s1v3.ckpt", "s2Gv2ProPlus.pth", reference)]
    assert runtime["preview_only"] is True
    assert runtime["reference_text"] == "Hello there"
    assert "actor_dir" not in runtime

    monkeypatch.setattr(
        provider,
        "synthesize_voice_actor",
        lambda _runtime, text, path: {"text": text, "path": str(path)},
    )
    output = tmp_path / "quick_voice_preview.wav"
    result = provider.synthesize_pretrained_voice_preview(runtime, "Test preview", output)
    assert result["preview_only"] is True
    assert result["path"] == str(output)

    with pytest.raises(provider.VoiceLabProviderError, match="preview_runtime_required"):
        provider.synthesize_pretrained_voice_preview({}, "Test", output)
    with pytest.raises(provider.VoiceLabProviderError, match="preview_output_name_invalid"):
        provider.synthesize_pretrained_voice_preview(runtime, "Test", tmp_path / "reference.wav")


@pytest.mark.parametrize("quality", [
    {"duration_ms": 2000.0},
    {"duration_ms": 11000.0},
    {"silence_fraction": 0.95},
    {"clipping_fraction": 0.07},
    {"active_rms": 100.0},
    {"dc_offset": 3000.0},
])
def test_pretrained_preview_rejects_unusable_recordings(tmp_path: Path, monkeypatch, quality) -> None:
    provider = load_provider_module()
    reference = tmp_path / "reference.wav"
    reference.write_bytes(b"placeholder")
    metrics = {
        "duration_ms": 5000.0,
        "silence_fraction": 0.10,
        "clipping_fraction": 0.0,
        "active_rms": 3000.0,
        "dc_offset": 0.0,
        **quality,
    }
    monkeypatch.setattr(provider, "require_regular_file", lambda *_args: (100, 1))
    monkeypatch.setattr(provider, "wav_signal_metrics_pcm16", lambda _path: metrics)
    monkeypatch.setattr(
        provider,
        "pretrained_voice_preview_assets",
        lambda _source: (_ for _ in ()).throw(AssertionError("must reject before model load")),
    )
    with pytest.raises(provider.VoiceLabProviderError, match="preview_reference_"):
        provider.prepare_pretrained_voice_preview(tmp_path, reference, "Hello there")
