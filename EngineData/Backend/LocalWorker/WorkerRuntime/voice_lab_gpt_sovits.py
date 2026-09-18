"""Pinned GPT-SoVITS V2ProPlus provider for VoiceLab build and trained-actor inference."""

from __future__ import annotations

import hashlib
import json
import os
import wave
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Iterator

from voice_lab_upstream_stage import install_headless_my_utils

ENGINE = "gpt-sovits-v2proplus"
ENGINE_REVISION = "d523079fc05d9a8028d6085bffe4a2757c32abb6"
VERSION = "v2ProPlus"
REFERENCE_MIN_MS = 3_000
REFERENCE_MAX_MS = 10_000
REFERENCE_TARGET_MS = 5_000
SOVITS_EPOCHS = 8
GPT_EPOCHS = 15
MAX_TRAINING_CANDIDATES = 3
ACTOR_SCHEMA_VERSION = 1
ACTOR_MANIFEST_FILE = "actor.json"
ACTOR_GPT_WEIGHT_FILE = "gpt.ckpt"
ACTOR_SOVITS_WEIGHT_FILE = "sovits.pth"
ACTOR_REFERENCE_WAV_FILE = "reference.wav"
MAX_ACTOR_MANIFEST_BYTES = 64 * 1024


class VoiceLabProviderError(RuntimeError):
    pass


class _CachedReferenceSpeakerModel:
    def __init__(self, base: Any, cached_embeddings: list[tuple[Any, Any]]) -> None:
        self._base = base
        self._cached_embeddings = cached_embeddings

    def __getattr__(self, name: str) -> Any:
        return getattr(self._base, name)

    def compute_embedding3(self, audio: Any) -> Any:
        for cached_audio, cached_embedding in self._cached_embeddings:
            if audio is cached_audio:
                return cached_embedding
        return self._base.compute_embedding3(audio)


def require_file(path: Path, label: str) -> None:
    if not path.is_file() or path.stat().st_size <= 0:
        raise VoiceLabProviderError(f"missing_asset:{label}")


def require_dir(path: Path, label: str) -> None:
    if not path.is_dir():
        raise VoiceLabProviderError(f"missing_asset:{label}")


def validate_source_revision(source_root: Path) -> Path:
    marker = source_root / "TRANSLATEIT_GPTSOVITS_REVISION.txt"
    require_file(marker, "revision_marker")
    if marker.read_text(encoding="utf-8").strip() != ENGINE_REVISION:
        raise VoiceLabProviderError("source_revision_mismatch")
    gsv = source_root / "GPT_SoVITS"
    require_dir(gsv, "GPT_SoVITS")
    return gsv


def inference_source_assets(source_root: Path) -> dict[str, Path]:
    gsv = validate_source_revision(source_root)
    assets = {
        "gsv": gsv,
        "hubert_model": gsv / "pretrained_models" / "chinese-hubert-base",
        "bert_model": gsv / "pretrained_models" / "chinese-roberta-wwm-ext-large",
        "sv_model": gsv / "pretrained_models" / "sv" / "pretrained_eres2netv2w24s4ep4.ckpt",
    }
    require_dir(assets["hubert_model"], "chinese_hubert_base")
    require_dir(assets["bert_model"], "chinese_bert_base")
    require_file(assets["sv_model"], "sv_model")
    nltk_root = source_root / "nltk_data"
    require_dir(nltk_root / "corpora" / "cmudict", "nltk_cmudict")
    require_dir(
        nltk_root / "taggers" / "averaged_perceptron_tagger",
        "nltk_averaged_perceptron_tagger",
    )
    require_dir(
        nltk_root / "taggers" / "averaged_perceptron_tagger_eng",
        "nltk_averaged_perceptron_tagger_eng",
    )
    return assets


def wav_duration_ms(path: Path) -> int:
    try:
        with wave.open(str(path), "rb") as reader:
            if (
                reader.getnchannels() != 1
                or reader.getsampwidth() != 2
                or reader.getframerate() != 32_000
            ):
                raise VoiceLabProviderError(f"noncanonical_take:{path.name}")
            frames = reader.getnframes()
    except VoiceLabProviderError:
        raise
    except Exception as exc:
        raise VoiceLabProviderError(f"invalid_take:{path.name}") from exc
    if frames <= 0:
        raise VoiceLabProviderError(f"empty_take:{path.name}")
    return frames * 1_000 // 32_000


def require_regular_file(path: Path, label: str) -> tuple[int, int]:
    if path.is_symlink() or not path.is_file():
        raise VoiceLabProviderError(f"invalid_actor_asset:{label}")
    stat = path.stat()
    if stat.st_size <= 0:
        raise VoiceLabProviderError(f"invalid_actor_asset:{label}")
    return int(stat.st_size), int(stat.st_mtime_ns)


def wav_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_actor_package(actor_dir: Path) -> dict[str, Any]:
    if actor_dir.is_symlink() or not actor_dir.is_dir():
        raise VoiceLabProviderError("approved_actor_missing")
    manifest_path = actor_dir / ACTOR_MANIFEST_FILE
    manifest_size, manifest_mtime = require_regular_file(manifest_path, "actor_manifest")
    if manifest_size > MAX_ACTOR_MANIFEST_BYTES:
        raise VoiceLabProviderError("actor_manifest_size_invalid")
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise VoiceLabProviderError("actor_manifest_invalid_json") from exc
    if not isinstance(manifest, dict):
        raise VoiceLabProviderError("actor_manifest_invalid_json")
    if (
        type(manifest.get("schema_version")) is not int
        or manifest["schema_version"] != ACTOR_SCHEMA_VERSION
    ):
        raise VoiceLabProviderError("actor_schema_mismatch")
    if manifest.get("engine") != ENGINE or manifest.get("engine_revision") != ENGINE_REVISION:
        raise VoiceLabProviderError("actor_engine_contract_mismatch")
    if (
        manifest.get("gpt_weight_file") != ACTOR_GPT_WEIGHT_FILE
        or manifest.get("sovits_weight_file") != ACTOR_SOVITS_WEIGHT_FILE
        or manifest.get("reference_wav_file") != ACTOR_REFERENCE_WAV_FILE
    ):
        raise VoiceLabProviderError("actor_package_filename_mismatch")
    reference_text = str(manifest.get("reference_text", "")).strip()
    if not reference_text:
        raise VoiceLabProviderError("actor_reference_text_missing")
    if manifest.get("held_out_evaluation_complete") is not True:
        raise VoiceLabProviderError("actor_evaluation_incomplete")
    reference_duration = manifest.get("reference_duration_ms")
    if type(reference_duration) is not int:
        raise VoiceLabProviderError("actor_reference_duration_invalid")
    gpt_path = actor_dir / ACTOR_GPT_WEIGHT_FILE
    sovits_path = actor_dir / ACTOR_SOVITS_WEIGHT_FILE
    reference_wav = actor_dir / ACTOR_REFERENCE_WAV_FILE
    gpt_identity = require_regular_file(gpt_path, "gpt_weight")
    sovits_identity = require_regular_file(sovits_path, "sovits_weight")
    reference_identity = require_regular_file(reference_wav, "reference_wav")
    duration_ms = wav_duration_ms(reference_wav)
    if (
        duration_ms < REFERENCE_MIN_MS
        or duration_ms > REFERENCE_MAX_MS
        or reference_duration != duration_ms
    ):
        raise VoiceLabProviderError("actor_reference_duration_invalid")
    fingerprint = (
        (ACTOR_MANIFEST_FILE, manifest_size, manifest_mtime),
        (ACTOR_GPT_WEIGHT_FILE, *gpt_identity),
        (ACTOR_SOVITS_WEIGHT_FILE, *sovits_identity),
        (ACTOR_REFERENCE_WAV_FILE, *reference_identity),
        ("reference_wav_sha256", wav_sha256(reference_wav)),
    )
    return {
        "actor_dir": actor_dir,
        "manifest": manifest,
        "gpt_path": gpt_path,
        "sovits_path": sovits_path,
        "reference_wav": reference_wav,
        "reference_text": reference_text,
        "reference_duration_ms": duration_ms,
        "fingerprint": fingerprint,
    }


def write_wav(path: Path, sample_rate: int, audio: Any) -> None:
    import numpy as np

    values = np.asarray(audio).reshape(-1)
    if values.dtype != np.int16:
        values = np.clip(values, -1.0, 1.0)
        values = (values * 32767.0).astype(np.int16)
    if values.size == 0:
        raise VoiceLabProviderError("evaluation_audio_empty")
    with wave.open(str(path), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(sample_rate)
        writer.writeframes(values.tobytes())


@contextmanager
def source_working_directory(source_root: Path) -> Iterator[None]:
    previous = Path.cwd()
    os.chdir(source_root)
    try:
        yield
    finally:
        os.chdir(previous)


def reference_speaker_embeddings(tts: Any) -> list[tuple[Any, Any]]:
    if not bool(getattr(tts, "is_v2pro", False)):
        return []
    prompt_cache = getattr(tts, "prompt_cache", {})
    references = prompt_cache.get("refer_spec", []) if isinstance(prompt_cache, dict) else []
    sv_model = getattr(tts, "sv_model", None)
    if sv_model is None:
        return []
    cached: list[tuple[Any, Any]] = []
    for entry in references:
        if not isinstance(entry, tuple) or len(entry) < 2 or entry[1] is None:
            continue
        audio = entry[1]
        cached.append((audio, sv_model.compute_embedding3(audio)))
    return cached


@contextmanager
def reuse_reference_speaker_embeddings(runtime: dict[str, Any]) -> Iterator[None]:
    tts = runtime.get("tts")
    cached = runtime.get("reference_speaker_embeddings")
    if tts is None or not isinstance(cached, list) or not cached:
        yield
        return
    original = getattr(tts, "sv_model", None)
    if original is None:
        yield
        return
    tts.sv_model = _CachedReferenceSpeakerModel(original, cached)
    try:
        yield
    finally:
        tts.sv_model = original


def create_tts_runtime(
    source_root: Path,
    assets: dict[str, Path],
    gpt_weight: Path,
    sovits_weight: Path,
    reference_wav: Path,
) -> dict[str, Any]:
    require_regular_file(gpt_weight, "gpt_weight")
    require_regular_file(sovits_weight, "sovits_weight")
    require_regular_file(reference_wav, "reference_wav")
    try:
        import torch

        cuda_available = bool(torch.cuda.is_available())
    except Exception as exc:
        raise VoiceLabProviderError(f"cuda_probe_failed:{type(exc).__name__}") from exc
    device = "cuda:0" if cuda_available else "cpu"
    with source_working_directory(source_root):
        install_headless_my_utils(source_root)
        saved_environment = {key: os.environ.get(key) for key in ("NLTK_DATA", "version")}
        os.environ["NLTK_DATA"] = str(source_root / "nltk_data")
        os.environ["version"] = VERSION
        try:
            from TTS_infer_pack.TTS import TTS, TTS_Config

            config = TTS_Config(
                {
                    "custom": {
                        "device": device,
                        "is_half": cuda_available,
                        "version": VERSION,
                        "t2s_weights_path": str(gpt_weight),
                        "vits_weights_path": str(sovits_weight),
                        "cnhuhbert_base_path": str(assets["hubert_model"]),
                        "bert_base_path": str(assets["bert_model"]),
                    }
                }
            )
            config.configs_path = str(
                source_root / "GPT_SoVITS" / "configs" / "translateit_tts_runtime.yaml"
            )
            tts = TTS(config)
            tts.set_ref_audio(str(reference_wav))
            cached_embeddings = reference_speaker_embeddings(tts)
        finally:
            for key, value in saved_environment.items():
                if value is None:
                    os.environ.pop(key, None)
                else:
                    os.environ[key] = value
    return {
        "tts": tts,
        "device": device,
        "reference_wav": reference_wav,
        "reference_cached": True,
        "reference_speaker_embeddings": cached_embeddings,
    }


def english_tts_inputs(text: str, reference_wav: Path, reference_text: str) -> dict[str, Any]:
    return {
        "text": text,
        "text_lang": "en",
        "ref_audio_path": str(reference_wav),
        "prompt_text": reference_text,
        "prompt_lang": "en",
        "batch_size": 1,
        "parallel_infer": False,
        "return_fragment": False,
        "streaming_mode": False,
        "seed": 233333,
    }


def load_voice_actor_runtime(source_root: Path, actor_dir: Path) -> dict[str, Any]:
    package = validate_actor_package(actor_dir)
    assets = inference_source_assets(source_root)
    runtime = create_tts_runtime(
        source_root,
        assets,
        package["gpt_path"],
        package["sovits_path"],
        package["reference_wav"],
    )
    runtime.update(
        {
            "actor_dir": actor_dir,
            "reference_text": package["reference_text"],
            "reference_duration_ms": package["reference_duration_ms"],
            "fingerprint": package["fingerprint"],
        }
    )
    return runtime


def synthesize_voice_actor(runtime: dict[str, Any], text: str, output_path: Path) -> dict[str, Any]:
    tts = runtime.get("tts")
    reference_wav = runtime.get("reference_wav")
    reference_text = str(runtime.get("reference_text", "")).strip()
    if tts is None or not isinstance(reference_wav, Path) or not reference_text:
        raise VoiceLabProviderError("voice_actor_runtime_invalid")
    with reuse_reference_speaker_embeddings(runtime):
        outputs = list(tts.run(english_tts_inputs(text, reference_wav, reference_text)))
    if len(outputs) != 1:
        raise VoiceLabProviderError(f"inference_output_count:{len(outputs)}")
    sample_rate, audio = outputs[0]
    write_wav(output_path, int(sample_rate), audio)
    if not output_path.is_file() or output_path.stat().st_size <= 44:
        raise VoiceLabProviderError("inference_audio_invalid")
    return {
        "sample_rate": int(sample_rate),
        "device": str(runtime.get("device", "unknown")),
        "reference_cached": bool(runtime.get("reference_cached")),
    }
