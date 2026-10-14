"""One-shot GPT-SoVITS V2ProPlus training/evaluation pipeline for VoiceLab.

This module is intentionally outside the live worker inference path. It owns
frozen-dataset preparation, bounded candidate training, held-out evaluation,
and candidate package construction for the Rust-owned one-shot build child.
"""

from __future__ import annotations

import gc
import hashlib
import json
import math
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable

from voice_lab_audio_quality import (
    attach_speaker_evidence,
    inspect_take_frames,
    reference_quiet_context_available,
)
from voice_lab_gpt_sovits import (
    ACTOR_GPT_WEIGHT_FILE,
    ACTOR_REFERENCE_WAV_FILE,
    ACTOR_SOVITS_WEIGHT_FILE,
    ENGINE,
    ENGINE_REVISION,
    GPT_EPOCHS,
    MAX_TRAINING_CANDIDATES,
    REFERENCE_MAX_MS,
    REFERENCE_MIN_MS,
    REFERENCE_TARGET_MS,
    SOVITS_EPOCHS,
    VERSION,
    VoiceLabProviderError,
    create_tts_runtime,
    english_tts_inputs,
    inference_source_assets,
    require_dir,
    require_file,
    wav_duration_ms,
    wav_signal_metrics_pcm16,
    write_wav,
)
from voice_lab_speaker_consistency import inspect_speaker_embeddings


def source_assets(source_root: Path) -> dict[str, Path]:
    assets = inference_source_assets(source_root)
    gsv = assets["gsv"]
    assets.update(
        {
            "text": gsv / "prepare_datasets" / "1-get-text.py",
            "hubert": gsv / "prepare_datasets" / "2-get-hubert-wav32k.py",
            "sv": gsv / "prepare_datasets" / "2-get-sv.py",
            "semantic": gsv / "prepare_datasets" / "3-get-semantic.py",
            "sovits_train": gsv / "s2_train.py",
            "gpt_train": gsv / "s1_train.py",
            "s2_config": gsv / "configs" / "s2v2ProPlus.json",
            "s1_config": gsv / "configs" / "s1longer-v2.yaml",
            "pretrained_gpt": gsv / "pretrained_models" / "s1v3.ckpt",
            "pretrained_sovits_g": gsv / "pretrained_models" / "v2Pro" / "s2Gv2ProPlus.pth",
            "pretrained_sovits_d": gsv / "pretrained_models" / "v2Pro" / "s2Dv2ProPlus.pth",
        }
    )
    for key in (
        "text",
        "hubert",
        "sv",
        "semantic",
        "sovits_train",
        "gpt_train",
        "s2_config",
        "s1_config",
        "pretrained_gpt",
        "pretrained_sovits_g",
        "pretrained_sovits_d",
    ):
        require_file(assets[key], key)
    require_file(source_root / "ffmpeg.exe", "ffmpeg")
    return assets


def training_takes(dataset_dir: Path, manifest: dict[str, Any]) -> list[dict[str, Any]]:
    result: list[dict[str, Any]] = []
    ids: set[int] = set()
    texts: set[str] = set()
    wav_files: set[str] = set()
    for item in manifest["takes"]:
        if not isinstance(item, dict):
            raise VoiceLabProviderError("invalid_training_take")
        line_id = int(item.get("line_id", 0))
        text = str(item.get("exact_text", "")).strip()
        wav_file = str(item.get("wav_file", "")).strip()
        if (
            line_id <= 0
            or not text
            or not wav_file
            or Path(wav_file).name != wav_file
            or line_id in ids
            or text in texts
            or wav_file in wav_files
        ):
            raise VoiceLabProviderError("invalid_training_take")
        wav_path = dataset_dir / wav_file
        result.append(
            {
                "line_id": line_id,
                "exact_text": text,
                "wav_file": wav_file,
                "wav_path": wav_path,
                "duration_ms": wav_duration_ms(wav_path),
            }
        )
        ids.add(line_id)
        texts.add(text)
        wav_files.add(wav_file)

    seen: set[int] = set()
    seen_texts: set[str] = set()
    for item in manifest["held_out_lines"]:
        if not isinstance(item, dict):
            raise VoiceLabProviderError("invalid_held_out_line")
        line_id = int(item.get("line_id", 0))
        text = str(item.get("exact_text", "")).strip()
        if (
            line_id <= 0
            or not text
            or line_id in ids
            or text in texts
            or line_id in seen
            or text in seen_texts
        ):
            raise VoiceLabProviderError("invalid_held_out_line")
        seen.add(line_id)
        seen_texts.add(text)
    return result


def select_reference(
    takes: list[dict[str, Any]],
    peer_cosines: dict[int, float] | None = None,
) -> dict[str, Any]:
    eligible = [x for x in takes if REFERENCE_MIN_MS <= int(x["duration_ms"]) <= REFERENCE_MAX_MS]
    if not eligible:
        raise VoiceLabProviderError("reference_take_3_to_10_seconds_required")

    # Only compare an edge-energy noise proxy if every eligible reference has
    # a sufficiently long quiet boundary. Normalize against each recording's
    # active level: an absolute PCM floor rewards turning down microphone gain
    # even when the speech/noise relationship did not improve. This is still
    # comparative energy evidence, NOT measured SNR or perceptual quality.
    # Without comparable edges, retain the existing validated signal ordering.
    evidence = [
        (
            take,
            wav_signal_metrics_pcm16(Path(take["wav_path"])),
            inspect_take_frames(Path(take["wav_path"])),
        )
        for take in eligible
    ]
    compare_quiet_edges = reference_quiet_context_available(
        [frames for _, _, frames in evidence]
    )
    ranked: list[tuple[tuple[float, ...], dict[str, Any]]] = []
    for take, metrics, frames in evidence:
        edge_floor = frames["edge_floor_rms_pcm16"]
        # Invalid/empty speech cannot earn a better normalized-noise rank.
        # Production build and Quick Preview already validate active signal.
        active_rms = float(metrics["active_rms"])
        relative_edge_floor = (
            float(edge_floor) / active_rms
            if compare_quiet_edges and edge_floor is not None and active_rms > 0.0
            else (float("inf") if compare_quiet_edges else 0.0)
        )
        ranked.append(
            (
                (
                    float(metrics["clipping_fraction"]),
                    relative_edge_floor,
                    float(metrics["silence_fraction"]),
                    float(metrics["dc_offset"]),
                    -float(peer_cosines.get(int(take["line_id"]), 0.0)) if peer_cosines else 0.0,
                    abs(int(take["duration_ms"]) - REFERENCE_TARGET_MS),
                    int(take["line_id"]),
                ),
                take,
            )
        )
    return min(ranked, key=lambda item: item[0])[1]


def stage_gpu_budget() -> int:
    """Recheck actual free VRAM immediately before a heavy child starts.

    A passed check is not a VRAM reservation or protection against DDP spawn.
    """
    from my_voice_gpu_policy import check_stage_budget
    from my_voice_gpu_identity import DeviceIdentityError, normalize_uuid

    try:
        expected_uuid = normalize_uuid(os.environ.get("TRANSLATEIT_MYVOICE_GPU_UUID", ""))
    except DeviceIdentityError as exc:
        raise VoiceLabProviderError("gpu_device_identity_missing") from exc
    if os.environ.get("CUDA_VISIBLE_DEVICES") != expected_uuid:
        raise VoiceLabProviderError("gpu_visible_device_mismatch")
    original_raw = os.environ.get("TRANSLATEIT_MYVOICE_GPU_BUDGET_MIB", "")
    if not original_raw.isdecimal():
        raise VoiceLabProviderError("gpu_budget_missing")
    original = int(original_raw)
    probe_path = Path(__file__).resolve().with_name("my_voice_compute_probe.py")
    completed = subprocess.run(
        [sys.executable, "-s", str(probe_path)],
        cwd=str(Path(__file__).resolve().parent),
        stdin=subprocess.DEVNULL, capture_output=True,
        timeout=20, check=False,
    )
    if completed.returncode != 0 or not completed.stdout or len(completed.stdout) > 4096:
        raise VoiceLabProviderError("gpu_recheck_unavailable")
    try:
        report = json.loads(completed.stdout)
        return check_stage_budget(report, original, expected_uuid)
    except (ValueError, TypeError, RuntimeError) as exc:
        raise VoiceLabProviderError("gpu_headroom_insufficient_or_unknown") from exc


def run_stage(
    source_root: Path, script: Path, env: dict[str, str], *args: str,
    status_writer: Callable[[str, str], None] | None = None,
) -> None:
    from my_voice_cooperative import manual_stop_requested
    try:
        if manual_stop_requested():
            raise VoiceLabProviderError("manual_stop_before_stage")
    except RuntimeError as exc:
        raise VoiceLabProviderError(f"manual_stop_unreadable:{exc}") from exc
    child_env = os.environ.copy()
    child_env.update(env)
    expected_uuid = os.environ.get("TRANSLATEIT_MYVOICE_GPU_UUID", "")
    if not expected_uuid or child_env.get("CUDA_VISIBLE_DEVICES") != expected_uuid:
        raise VoiceLabProviderError("gpu_stage_device_mismatch")
    # The English text step does not need CUDA; every remaining stage does.
    if script.name != "1-get-text.py":
        budget_mib = stage_gpu_budget()
        child_env["TRANSLATEIT_MYVOICE_STAGE_GPU_BUDGET_MIB"] = str(budget_mib)
    child_env["PYTHONNOUSERSITE"] = "1"
    child_env["NLTK_DATA"] = str(source_root / "nltk_data")
    runner = Path(__file__).resolve().with_name("voice_lab_upstream_stage.py")
    require_file(runner, "headless_stage_runner")
    from my_voice_stage_supervisor import StageGuardError, run_supervised_stage

    def safety_change(state: str, reason: str) -> None:
        if status_writer is None:
            return
        stage = (
            "SoVITS training stage" if script.name == "s2_train.py"
            else "GPT training stage" if script.name == "s1_train.py"
            else script.name
        )
        if state == "watch":
            message = f"{stage}: resource use is high or changing; continuing under observation."
        elif state == "pressure":
            message = f"{stage}: resources are declining; requesting a checkpoint-boundary stop."
        elif state in ("critical", "unknown"):
            message = f"{stage}: critical or unknown resources; emergency protection active."
        else:
            message = f"{stage}: resource pressure has returned to normal."
        status_writer("training", message)

    def heartbeat(state: str) -> None:
        if status_writer is not None:
            stage_name = "SoVITS" if script.name == "s2_train.py" else (
                "GPT" if script.name == "s1_train.py" else script.name
            )
            status_writer("training", f"{stage_name} stage running; resource safety: {state}.")

    try:
        run_supervised_stage(
            [sys.executable, "-s", str(runner), str(source_root), str(script), *args],
            cwd=str(source_root), env=child_env,
            gpu_stage=script.name != "1-get-text.py",
            cooperative_stage=(
                "sovits" if script.name == "s2_train.py"
                else "gpt" if script.name == "s1_train.py" else None
            ),
            on_safety_change=safety_change if status_writer is not None else None,
            on_heartbeat=heartbeat if status_writer is not None else None,
        )
    except StageGuardError as exc:
        raise VoiceLabProviderError(f"upstream_stage_guard:{script.name}:{exc}") from exc


def merge_part(source: Path, target: Path, header: str | None = None) -> None:
    require_file(source, source.name)
    body = source.read_text(encoding="utf-8").strip()
    if not body:
        raise VoiceLabProviderError(f"upstream_output_empty:{source.name}")
    target.write_text(
        (f"{header}\n" if header else "") + body + "\n",
        encoding="utf-8",
        newline="\n",
    )
    source.unlink(missing_ok=True)


def prepare_dataset(
    source_root: Path,
    assets: dict[str, Path],
    dataset_dir: Path,
    exp: Path,
    takes: list[dict[str, Any]],
) -> None:
    exp.mkdir(parents=True, exist_ok=True)
    list_path = exp.parent / "translateit.list"
    list_path.write_text(
        "\n".join(f"{x['wav_file']}|MyVoice|en|{x['exact_text']}" for x in takes) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    common = {
        "inp_text": str(list_path),
        "inp_wav_dir": str(dataset_dir),
        "exp_name": "translateit_myvoice",
        "opt_dir": str(exp),
        "i_part": "0",
        "all_parts": "1",
        "is_half": "True",
        "version": VERSION,
    }
    env = {**common, "bert_pretrained_dir": str(assets["bert_model"])}
    run_stage(source_root, assets["text"], env)
    merge_part(exp / "2-name2text-0.txt", exp / "2-name2text.txt")

    env = {**common, "cnhubert_base_dir": str(assets["hubert_model"])}
    run_stage(source_root, assets["hubert"], env)
    env = {**common, "sv_path": str(assets["sv_model"])}
    run_stage(source_root, assets["sv"], env)
    require_dir(exp / "7-sv_cn", "speaker_embedding_output")

    env = {
        **common,
        "pretrained_s2G": str(assets["pretrained_sovits_g"]),
        "s2config_path": str(assets["s2_config"]),
    }
    run_stage(source_root, assets["semantic"], env)
    merge_part(
        exp / "6-name2semantic-0.tsv",
        exp / "6-name2semantic.tsv",
        "item_name\tsemantic_audio",
    )


def batch_and_half() -> tuple[int, bool]:
    import torch

    if not torch.cuda.is_available():
        raise VoiceLabProviderError("safe_cuda_requires_supported_gpu")
    from my_voice_precision_policy import device_precision_policy
    _cc, precision = device_precision_policy(torch.cuda, 0)
    if precision != "mixed_fp16":
        raise VoiceLabProviderError("safe_cuda_pinned_mixed_precision_unsupported")
    # A physical batch of one prevents the previous 8 GB -> batch 4 heuristic.
    # This is a conservative attempt, NOT verified OOM-free training.
    return 1, True


def training_configs(
    assets: dict[str, Path], exp: Path, sovits_dir: Path, gpt_dir: Path
) -> tuple[Path, Path, dict[str, str]]:
    import yaml

    from my_voice_gpu_identity import DeviceIdentityError, normalize_uuid
    try:
        physical_uuid = normalize_uuid(os.environ["TRANSLATEIT_MYVOICE_GPU_UUID"])
    except (KeyError, DeviceIdentityError) as exc:
        raise VoiceLabProviderError("gpu_uuid_unavailable_for_training") from exc
    if os.environ.get("CUDA_VISIBLE_DEVICES") != physical_uuid:
        raise VoiceLabProviderError("gpu_uuid_changed_before_training")
    batch, half = batch_and_half()
    sovits_save_every = max(1, SOVITS_EPOCHS // MAX_TRAINING_CANDIDATES)
    gpt_save_every = max(1, GPT_EPOCHS // MAX_TRAINING_CANDIDATES)

    s2 = json.loads(assets["s2_config"].read_text(encoding="utf-8"))
    s2["train"].update(
        {
            "batch_size": batch if half else max(1, batch // 2),
            "epochs": SOVITS_EPOCHS,
            "text_low_lr_rate": 0.4,
            "pretrained_s2G": str(assets["pretrained_sovits_g"]),
            "pretrained_s2D": str(assets["pretrained_sovits_d"]),
            # Keep distinct full G/D optimizer checkpoints; never overwrite
            # the only recoverable pair with an incomplete new save.
            "if_save_latest": False,
            "if_save_every_weights": True,
            "save_every_epoch": sovits_save_every,
            "gpu_numbers": "0",  # Only local ordinal after physical UUID scoping
            "grad_ckpt": False,
            "lora_rank": "32",
            "fp16_run": half,
        }
    )
    s2["model"]["version"] = VERSION
    s2["data"]["exp_dir"] = str(exp)
    s2["s2_ckpt_dir"] = str(exp)
    s2["save_weight_dir"] = str(sovits_dir)
    s2["name"] = "translateit_myvoice"
    s2["version"] = VERSION
    s2_path = exp.parent / "translateit_s2.json"
    s2_path.write_text(json.dumps(s2, indent=2) + "\n", encoding="utf-8", newline="\n")

    s1 = yaml.safe_load(assets["s1_config"].read_text(encoding="utf-8"))
    s1["train"].update(
        {
            "precision": "16-mixed" if half else "32",
            "batch_size": batch if half else max(1, batch // 2),
            "epochs": GPT_EPOCHS,
            "save_every_n_epoch": gpt_save_every,
            "if_save_every_weights": True,
            # Lightning saves every configured epoch checkpoint; do not
            # prune older valid full checkpoints before the next is verified.
            "if_save_latest": False,
            "if_dpo": False,
            "half_weights_save_dir": str(gpt_dir),
            "exp_name": "translateit_myvoice",
        }
    )
    s1["pretrained_s1"] = str(assets["pretrained_gpt"])
    s1["train_semantic_path"] = str(exp / "6-name2semantic.tsv")
    s1["train_phoneme_path"] = str(exp / "2-name2text.txt")
    s1["output_dir"] = str(exp / f"logs_s1_{VERSION}")
    s1_path = exp.parent / "translateit_s1.yaml"
    s1_path.write_text(yaml.safe_dump(s1, sort_keys=False), encoding="utf-8", newline="\n")
    return (
        s2_path,
        s1_path,
        {
            "version": VERSION,
            "hz": "25hz",
            "is_half": str(half),
            "_CUDA_VISIBLE_DEVICES": physical_uuid,
        },
    )


def checkpoint_epoch(path: Path, family: str) -> int:
    if family == "sovits":
        match = re.search(r"_e(\d+)_s\d+\.pth$", path.name)
    elif family == "gpt":
        match = re.search(r"-e(\d+)\.ckpt$", path.name)
    else:
        raise VoiceLabProviderError(f"unknown_candidate_family:{family}")
    if not match:
        raise VoiceLabProviderError(f"{family}_checkpoint_epoch_unparseable:{path.name}")
    return int(match.group(1))


def epoch_weights(directory: Path, suffix: str, family: str) -> dict[int, Path]:
    result: dict[int, Path] = {}
    for path in directory.glob(f"*{suffix}"):
        if not path.is_file() or path.stat().st_size <= 0:
            continue
        epoch = checkpoint_epoch(path, family)
        if epoch in result:
            raise VoiceLabProviderError(f"{family}_duplicate_checkpoint_epoch:{epoch}")
        result[epoch] = path
    if not result:
        raise VoiceLabProviderError(f"{family}_candidate_checkpoints_missing")
    return result


def nearest_epoch(checkpoints: dict[int, Path], total_epochs: int, progress_numerator: int) -> int:
    if not checkpoints or total_epochs <= 0:
        raise VoiceLabProviderError("candidate_checkpoint_set_invalid")
    denominator = MAX_TRAINING_CANDIDATES
    target = total_epochs * progress_numerator
    return min(checkpoints, key=lambda epoch: (abs(epoch * denominator - target), -epoch))


def select_training_candidates(sovits_dir: Path, gpt_dir: Path) -> list[dict[str, Any]]:
    sovits = epoch_weights(sovits_dir, ".pth", "sovits")
    gpt = epoch_weights(gpt_dir, ".ckpt", "gpt")
    candidates: list[dict[str, Any]] = []
    seen_pairs: set[tuple[int, int]] = set()

    for progress_numerator in range(1, MAX_TRAINING_CANDIDATES + 1):
        sovits_epoch = nearest_epoch(sovits, SOVITS_EPOCHS, progress_numerator)
        gpt_epoch = nearest_epoch(gpt, GPT_EPOCHS, progress_numerator)
        pair = (sovits_epoch, gpt_epoch)
        if pair in seen_pairs:
            continue
        seen_pairs.add(pair)
        candidates.append(
            {
                "candidate_id": f"s{sovits_epoch}-g{gpt_epoch}",
                "candidate_order": len(candidates),
                "sovits_epoch": sovits_epoch,
                "gpt_epoch": gpt_epoch,
                "sovits_path": sovits[sovits_epoch],
                "gpt_path": gpt[gpt_epoch],
            }
        )

    if len(candidates) < 2:
        raise VoiceLabProviderError(f"candidate_checkpoint_set_too_small:{len(candidates)}")
    return candidates[:MAX_TRAINING_CANDIDATES]


def train(
    source_root: Path,
    assets: dict[str, Path],
    exp: Path,
    candidate: Path,
    status_writer: Callable[[str, str], None],
) -> list[dict[str, Any]]:
    sovits_dir = candidate / "_sovits_weights"
    gpt_dir = candidate / "_gpt_weights"
    sovits_dir.mkdir(parents=True, exist_ok=True)
    gpt_dir.mkdir(parents=True, exist_ok=True)
    s2, s1, env = training_configs(assets, exp, sovits_dir, gpt_dir)
    # The headless runner applies the pinned single-GPU adapter. No upstream
    # source files are edited and source-hash drift blocks training.
    status_writer("training", "Learning voice characteristics (SoVITS training stage).")
    run_stage(source_root, assets["sovits_train"], env, "--config", str(s2),
              status_writer=status_writer)
    status_writer("training", "Learning speech generation (GPT training stage).")
    run_stage(source_root, assets["gpt_train"], env, "--config_file", str(s1),
              status_writer=status_writer)
    return select_training_candidates(sovits_dir, gpt_dir)


def embedding(tts: Any, wav_path: Path) -> Any:
    import torchaudio

    wav, sr = torchaudio.load(str(wav_path))
    wav = wav.mean(dim=0, keepdim=True)
    if sr != 16_000:
        wav = torchaudio.functional.resample(wav, sr, 16_000)
    return tts.sv_model.compute_embedding3(wav.to(tts.configs.device)).detach().float().cpu()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def normalized_words(text: str) -> list[str]:
    return re.findall(r"[a-z0-9]+(?:'[a-z0-9]+)?", text.casefold())


def word_error_rate(reference: str, hypothesis: str) -> float:
    expected = normalized_words(reference)
    actual = normalized_words(hypothesis)
    if not expected:
        return 0.0 if not actual else 1.0
    previous = list(range(len(actual) + 1))
    for row, expected_word in enumerate(expected, start=1):
        current = [row]
        for column, actual_word in enumerate(actual, start=1):
            substitution = previous[column - 1] + (expected_word != actual_word)
            insertion = current[column - 1] + 1
            deletion = previous[column] + 1
            current.append(min(substitution, insertion, deletion))
        previous = current
    return previous[-1] / len(expected)


def synthesis_artifact_flags(audio: Any, sample_rate: int = 32_000) -> list[str]:
    import numpy as np

    values = np.asarray(audio).reshape(-1)
    if values.size == 0:
        return ["unexpected_silence"]
    if np.issubdtype(values.dtype, np.integer):
        scale = float(max(abs(np.iinfo(values.dtype).min), np.iinfo(values.dtype).max))
        normalized = values.astype(np.float32) / max(scale, 1.0)
    else:
        normalized = values.astype(np.float32)
    finite = np.nan_to_num(normalized, nan=0.0, posinf=1.0, neginf=-1.0)
    absolute = np.abs(finite)
    flags: list[str] = []
    if float(np.mean(absolute >= 0.98)) >= 0.05:
        flags.append("clipping")
    silence_mask = absolute <= (128.0 / 32_768.0)
    silence_fraction = float(np.mean(silence_mask))
    if silence_fraction >= 0.90:
        flags.append("unexpected_silence")
    elif sample_rate > 0:
        longest_silence = 0
        current_silence = 0
        for silent in silence_mask:
            if bool(silent):
                current_silence += 1
                longest_silence = max(longest_silence, current_silence)
            else:
                current_silence = 0
        if longest_silence >= sample_rate * 3:
            flags.append("dropout")
    return flags


def evaluation_asr_model_path(asr_model_root: Path) -> Path:
    for dirname in ("faster-whisper-large-v3-turbo", "faster-whisper-medium"):
        candidate = asr_model_root / dirname
        if (candidate / "model.bin").is_file() and (candidate / "config.json").is_file():
            return candidate
    raise VoiceLabProviderError("evaluation_asr_model_missing")


def create_evaluation_asr_runtime(asr_model_root: Path) -> Any:
    from faster_whisper import WhisperModel

    model_path = evaluation_asr_model_path(asr_model_root)
    return WhisperModel(str(model_path), device="cpu", compute_type="int8")


def transcribe_evaluation_audio(asr_model: Any, wav_path: Path) -> str:
    segments, _info = asr_model.transcribe(
        str(wav_path),
        language="en",
        task="transcribe",
        beam_size=3,
        temperature=0.0,
        condition_on_previous_text=False,
        vad_filter=False,
        without_timestamps=True,
        word_timestamps=False,
    )
    return " ".join(str(segment.text).strip() for segment in segments).strip()


def evaluate_candidate(
    source_root: Path,
    assets: dict[str, Path],
    candidate: dict[str, Any],
    evaluation_root: Path,
    manifest: dict[str, Any],
    reference: dict[str, Any],
    asr_model: Any,
) -> dict[str, Any]:
    import torch
    import torch.nn.functional as functional

    candidate_id = str(candidate["candidate_id"])
    candidate_dir = evaluation_root / candidate_id
    candidate_dir.mkdir(parents=True, exist_ok=True)
    reference_wav = Path(reference["wav_path"])
    runtime = create_tts_runtime(
        source_root,
        assets,
        Path(candidate["gpt_path"]),
        Path(candidate["sovits_path"]),
        reference_wav,
    )
    tts = runtime["tts"]
    samples: list[dict[str, Any]] = []
    try:
        ref_embedding = embedding(tts, reference_wav)
        for held in manifest["held_out_lines"]:
            line_id = int(held["line_id"])
            held_text = str(held["exact_text"]).strip()
            outputs = list(
                tts.run(
                    english_tts_inputs(
                        held_text,
                        reference_wav,
                        str(reference["exact_text"]),
                    )
                )
            )
            if len(outputs) != 1:
                raise VoiceLabProviderError(
                    f"evaluation_output_count:{candidate_id}:{line_id}:{len(outputs)}"
                )
            sr, audio = outputs[0]
            artifact_flags = synthesis_artifact_flags(audio, int(sr))
            wav_path = candidate_dir / f"held_out_{line_id}.wav"
            write_wav(wav_path, int(sr), audio)
            score = float(
                functional.cosine_similarity(
                    ref_embedding,
                    embedding(tts, wav_path),
                    dim=-1,
                )
                .mean()
                .item()
            )
            if not math.isfinite(score):
                raise VoiceLabProviderError(
                    f"evaluation_similarity_invalid:{candidate_id}:{line_id}"
                )
            intelligibility_text = transcribe_evaluation_audio(asr_model, wav_path)
            if not intelligibility_text:
                raise VoiceLabProviderError(
                    f"evaluation_intelligibility_empty:{candidate_id}:{line_id}"
                )
            intelligibility_wer = word_error_rate(held_text, intelligibility_text)
            if not math.isfinite(intelligibility_wer):
                raise VoiceLabProviderError(
                    f"evaluation_intelligibility_invalid:{candidate_id}:{line_id}"
                )
            samples.append(
                {
                    "line_id": line_id,
                    "exact_text": held_text,
                    "speaker_similarity": round(score, 6),
                    "intelligibility_text": intelligibility_text,
                    "intelligibility_wer": round(intelligibility_wer, 6),
                    "artifact_flags": artifact_flags,
                    "sha256": sha256_file(wav_path),
                    "_wav_path": wav_path,
                }
            )
    finally:
        del tts
        del runtime
        gc.collect()
        if torch.cuda.is_available():
            torch.cuda.empty_cache()

    if len(samples) != len(manifest["held_out_lines"]):
        raise VoiceLabProviderError(f"held_out_evaluation_incomplete:{candidate_id}")
    similarities = [float(sample["speaker_similarity"]) for sample in samples]
    wers = [float(sample["intelligibility_wer"]) for sample in samples]
    artifact_case_count = sum(bool(sample["artifact_flags"]) for sample in samples)
    return {
        "candidate_id": candidate_id,
        "candidate_order": int(candidate["candidate_order"]),
        "sovits_epoch": int(candidate["sovits_epoch"]),
        "gpt_epoch": int(candidate["gpt_epoch"]),
        "sovits_path": Path(candidate["sovits_path"]),
        "gpt_path": Path(candidate["gpt_path"]),
        "mean_speaker_similarity": round(sum(similarities) / len(similarities), 6),
        "minimum_speaker_similarity": round(min(similarities), 6),
        "mean_intelligibility_wer": round(sum(wers) / len(wers), 6),
        "maximum_intelligibility_wer": round(max(wers), 6),
        "artifact_case_count": artifact_case_count,
        "samples": samples,
    }


def select_best_candidate(evidence: list[dict[str, Any]]) -> dict[str, Any]:
    if len(evidence) < 2 or len(evidence) > MAX_TRAINING_CANDIDATES:
        raise VoiceLabProviderError(f"candidate_evidence_count_invalid:{len(evidence)}")
    for candidate in evidence:
        samples = candidate.get("samples")
        mean_similarity = candidate.get("mean_speaker_similarity")
        minimum_similarity = candidate.get("minimum_speaker_similarity")
        mean_wer = candidate.get("mean_intelligibility_wer")
        maximum_wer = candidate.get("maximum_intelligibility_wer")
        artifact_case_count = candidate.get("artifact_case_count")
        if not isinstance(samples, list) or not samples:
            raise VoiceLabProviderError("candidate_evidence_samples_missing")
        if not isinstance(mean_similarity, (int, float)) or not math.isfinite(
            float(mean_similarity)
        ):
            raise VoiceLabProviderError("candidate_evidence_mean_invalid")
        if not isinstance(minimum_similarity, (int, float)) or not math.isfinite(
            float(minimum_similarity)
        ):
            raise VoiceLabProviderError("candidate_evidence_minimum_invalid")
        if not isinstance(mean_wer, (int, float)) or not math.isfinite(float(mean_wer)):
            raise VoiceLabProviderError("candidate_evidence_mean_wer_invalid")
        if not isinstance(maximum_wer, (int, float)) or not math.isfinite(float(maximum_wer)):
            raise VoiceLabProviderError("candidate_evidence_maximum_wer_invalid")
        if type(artifact_case_count) is not int or artifact_case_count < 0:
            raise VoiceLabProviderError("candidate_evidence_artifact_count_invalid")
    selected = min(
        evidence,
        key=lambda candidate: (
            int(candidate["artifact_case_count"]),
            float(candidate["mean_intelligibility_wer"]),
            float(candidate["maximum_intelligibility_wer"]),
            -float(candidate["mean_speaker_similarity"]),
            -float(candidate["minimum_speaker_similarity"]),
            int(candidate["candidate_order"]),
        ),
    )
    if int(selected["artifact_case_count"]) > 0:
        raise VoiceLabProviderError("candidate_artifact_free_checkpoint_missing")
    return selected


def public_candidate_evidence(candidate: dict[str, Any]) -> dict[str, Any]:
    return {
        "candidate_id": str(candidate["candidate_id"]),
        "sovits_epoch": int(candidate["sovits_epoch"]),
        "gpt_epoch": int(candidate["gpt_epoch"]),
        "mean_speaker_similarity": float(candidate["mean_speaker_similarity"]),
        "minimum_speaker_similarity": float(candidate["minimum_speaker_similarity"]),
        "mean_intelligibility_wer": float(candidate["mean_intelligibility_wer"]),
        "maximum_intelligibility_wer": float(candidate["maximum_intelligibility_wer"]),
        "artifact_case_count": int(candidate["artifact_case_count"]),
        "samples": [
            {
                "line_id": int(sample["line_id"]),
                "exact_text": str(sample["exact_text"]),
                "speaker_similarity": float(sample["speaker_similarity"]),
                "intelligibility_text": str(sample["intelligibility_text"]),
                "intelligibility_wer": float(sample["intelligibility_wer"]),
                "artifact_flags": list(sample["artifact_flags"]),
                "sha256": str(sample["sha256"]),
            }
            for sample in candidate["samples"]
        ],
    }


def promote_selected_candidate(
    selected: dict[str, Any],
    candidate_dir: Path,
    evaluation_dir: Path,
    reference: dict[str, Any],
) -> list[dict[str, Any]]:
    shutil.copy2(Path(selected["gpt_path"]), candidate_dir / ACTOR_GPT_WEIGHT_FILE)
    shutil.copy2(Path(selected["sovits_path"]), candidate_dir / ACTOR_SOVITS_WEIGHT_FILE)
    shutil.copy2(Path(reference["wav_path"]), candidate_dir / ACTOR_REFERENCE_WAV_FILE)

    selected_samples: list[dict[str, Any]] = []
    for sample in selected["samples"]:
        line_id = int(sample["line_id"])
        wav_file = f"held_out_{line_id}.wav"
        target = evaluation_dir / wav_file
        shutil.copy2(Path(sample["_wav_path"]), target)
        if sha256_file(target) != str(sample["sha256"]):
            raise VoiceLabProviderError(f"selected_evaluation_copy_hash_mismatch:{line_id}")
        selected_samples.append(
            {
                "line_id": line_id,
                "exact_text": str(sample["exact_text"]),
                "wav_file": wav_file,
                "speaker_similarity": float(sample["speaker_similarity"]),
                "intelligibility_text": str(sample["intelligibility_text"]),
                "intelligibility_wer": float(sample["intelligibility_wer"]),
                "artifact_flags": list(sample["artifact_flags"]),
                "sha256": str(sample["sha256"]),
            }
        )
    return selected_samples


def require_training_not_cancelled(boundary: str) -> None:
    """One run-scoped cancellation check at expensive pipeline boundaries."""
    from my_voice_cooperative import manual_stop_requested

    try:
        if manual_stop_requested():
            raise VoiceLabProviderError(f"manual_stop_at:{boundary}")
    except RuntimeError as exc:
        raise VoiceLabProviderError(f"manual_stop_signal_unavailable_at:{boundary}") from exc


def build_candidate(
    *,
    source_root: Path,
    asr_model_root: Path,
    dataset_dir: Path,
    candidate_dir: Path,
    evaluation_dir: Path,
    work_dir: Path,
    manifest: dict[str, Any],
    status_writer: Callable[[str, str], None],
) -> None:
    require_training_not_cancelled("candidate_start")
    assets = source_assets(source_root)
    takes = training_takes(dataset_dir, manifest)
    for path in (candidate_dir, evaluation_dir, work_dir):
        if path.exists():
            shutil.rmtree(path)
        path.mkdir(parents=True, exist_ok=True)

    exp = work_dir / "experiment"
    require_training_not_cancelled("feature_extraction")
    status_writer("preparing", "Extracting audio features from accepted recordings.")
    prepare_dataset(source_root, assets, dataset_dir, exp, takes)
    # Reuse artifacts produced by the pinned SoVITS SV preprocessing stage.
    speaker_evidence = inspect_speaker_embeddings(exp / "7-sv_cn", takes)
    try:
        attach_speaker_evidence(dataset_dir / "audio_quality.json", takes, speaker_evidence)
    except ValueError as exc:
        raise VoiceLabProviderError(f"speaker_quality_report_failed:{exc}") from exc
    peer_cosines = {
        int(item["line_id"]): float(item["median_peer_cosine"])
        for item in speaker_evidence["per_take"]
    }
    reference = select_reference(takes, peer_cosines=peer_cosines)
    require_training_not_cancelled("training_start")
    candidates = train(source_root, assets, exp, candidate_dir, status_writer)

    status_writer(
        "evaluating",
        "Comparing held-out Voice Actor candidates before review.",
    )
    require_training_not_cancelled("evaluation_start")
    candidate_evaluation_root = work_dir / "candidate_evaluation"
    candidate_evaluation_root.mkdir(parents=True, exist_ok=True)
    from my_voice_stage_supervisor import StageGuardError, run_supervised_stage

    evidence: list[dict[str, Any]] = []
    evaluator = Path(__file__).resolve().with_name("my_voice_evaluation_child.py")
    require_file(evaluator, "evaluation_child")
    for index, candidate in enumerate(candidates, start=1):
        require_training_not_cancelled("evaluation_next_candidate")
        status_writer(
            "evaluating",
            f"Evaluating candidate {index} of {len(candidates)} in an isolated GPU process.",
        )
        # A previous model's CUDA allocations are released when its child exits.
        # Never retry the same candidate automatically after a VRAM or native failure.
        safe_id = str(candidate["candidate_id"])
        candidate_data = {key: str(value) if isinstance(value, Path) else value
                          for key, value in candidate.items()}
        child_request = {
            "source_root": str(source_root),
            "asr_model_root": str(asr_model_root),
            "evaluation_root": str(candidate_evaluation_root),
            "candidate": candidate_data,
            "manifest": manifest,
            "reference": {k: str(v) if isinstance(v, Path) else v for k, v in reference.items()},
        }
        request_path = candidate_evaluation_root / f"{safe_id}.request.json"
        report_path = candidate_evaluation_root / f"{safe_id}.evidence.json"
        if request_path.exists() or report_path.exists():
            raise VoiceLabProviderError("evaluation_child_input_collision")
        request_path.write_text(
            json.dumps(child_request, ensure_ascii=True, separators=(",", ":")),
            encoding="utf-8", newline="\n",
        )
        budget = stage_gpu_budget()
        env = os.environ.copy()
        env["TRANSLATEIT_MYVOICE_STAGE_GPU_BUDGET_MIB"] = str(budget)
        try:
            run_supervised_stage(
                [sys.executable, "-s", str(evaluator),
                 "--request", str(request_path), "--result", str(report_path)],
                cwd=str(Path(__file__).resolve().parent), env=env, gpu_stage=True,
            )
        except StageGuardError as exc:
            raise VoiceLabProviderError(f"evaluation_child_guard:{safe_id}:{exc}") from exc
        if report_path.is_symlink() or not report_path.is_file() or report_path.stat().st_size > 96 * 1024:
            raise VoiceLabProviderError("evaluation_evidence_missing")
        try:
            item = json.loads(report_path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            raise VoiceLabProviderError("evaluation_evidence_invalid") from exc
        if not isinstance(item, dict) or item.get("candidate_id") != safe_id:
            raise VoiceLabProviderError("evaluation_candidate_identity_mismatch")
        if Path(str(item.get("gpt_path", ""))) != Path(candidate["gpt_path"]) or (
            Path(str(item.get("sovits_path", ""))) != Path(candidate["sovits_path"])
        ):
            raise VoiceLabProviderError("evaluation_candidate_weight_mismatch")
        samples = item.get("samples")
        held_out = manifest["held_out_lines"]
        if not isinstance(samples, list) or len(samples) != len(held_out):
            raise VoiceLabProviderError("evaluation_samples_incomplete")
        for sample, expected in zip(samples, held_out):
            line_id = int(expected["line_id"])
            wav = candidate_evaluation_root / safe_id / f"held_out_{line_id}.wav"
            if (not isinstance(sample, dict) or sample.get("line_id") != line_id
                or sample.get("exact_text") != expected["exact_text"]
                or sample.get("sha256") != sha256_file(wav)
                or wav.is_symlink()):
                raise VoiceLabProviderError("evaluation_sample_identity_invalid")
            sample["_wav_path"] = wav
        item["gpt_path"] = Path(candidate["gpt_path"])
        item["sovits_path"] = Path(candidate["sovits_path"])
        evidence.append(item)
    require_training_not_cancelled("candidate_selection")
    selected = select_best_candidate(evidence)
    selected_samples = promote_selected_candidate(
        selected,
        candidate_dir,
        evaluation_dir,
        reference,
    )

    selection_method = "held_out_artifacts_then_mean_wer_then_max_wer_then_similarity_tiebreak"
    # Review identity uses the actual selected WAV hashes; held-out line IDs and
    # checkpoint labels are reused across builds and cannot identify audio.
    review_fingerprint = {
        "engine_revision": ENGINE_REVISION,
        "selection_method": selection_method,
        "candidate_id": str(selected["candidate_id"]),
        "audio": [
            [int(sample["line_id"]), str(sample["sha256"])]
            for sample in selected["samples"]
        ],
    }
    review_id = hashlib.sha256(
        json.dumps(review_fingerprint, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    evaluation_payload = {
        "review_id": review_id,
        "schema_version": 1,
        "engine": ENGINE,
        "engine_revision": ENGINE_REVISION,
        "selection_method": selection_method,
        "selected_candidate_id": str(selected["candidate_id"]),
        "selected_candidate": public_candidate_evidence(selected),
        "candidate_evidence": [public_candidate_evidence(candidate) for candidate in evidence],
        "samples": selected_samples,
    }
    (evaluation_dir / "evaluation.json").write_text(
        json.dumps(evaluation_payload, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    actor_payload = {
        "schema_version": 1,
        "engine": ENGINE,
        "engine_revision": ENGINE_REVISION,
        "gpt_weight_file": ACTOR_GPT_WEIGHT_FILE,
        "sovits_weight_file": ACTOR_SOVITS_WEIGHT_FILE,
        "reference_wav_file": ACTOR_REFERENCE_WAV_FILE,
        "reference_text": reference["exact_text"],
        "reference_duration_ms": int(reference["duration_ms"]),
        "held_out_evaluation_complete": True,
        "candidate_selection": {
            "method": selection_method,
            "candidate_id": str(selected["candidate_id"]),
            "sovits_epoch": int(selected["sovits_epoch"]),
            "gpt_epoch": int(selected["gpt_epoch"]),
            "mean_speaker_similarity": float(selected["mean_speaker_similarity"]),
            "minimum_speaker_similarity": float(selected["minimum_speaker_similarity"]),
            "mean_intelligibility_wer": float(selected["mean_intelligibility_wer"]),
            "maximum_intelligibility_wer": float(selected["maximum_intelligibility_wer"]),
            "artifact_case_count": int(selected["artifact_case_count"]),
        },
    }
    (candidate_dir / "actor.json").write_text(
        json.dumps(actor_payload, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )

    shutil.rmtree(candidate_evaluation_root, ignore_errors=True)
    shutil.rmtree(candidate_dir / "_sovits_weights", ignore_errors=True)
    shutil.rmtree(candidate_dir / "_gpt_weights", ignore_errors=True)
