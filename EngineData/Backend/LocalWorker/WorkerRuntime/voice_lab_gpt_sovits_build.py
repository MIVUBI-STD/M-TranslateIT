"""One-shot GPT-SoVITS V2ProPlus training/evaluation pipeline for VoiceLab.

This module is intentionally outside the live worker inference path. It owns
frozen-dataset preparation, bounded candidate training, held-out evaluation,
and candidate package construction for the Rust-owned one-shot build child.
"""

from __future__ import annotations

import gc
import json
import math
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable

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
    write_wav,
)

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
    for item in manifest["takes"]:
        if not isinstance(item, dict):
            raise VoiceLabProviderError("invalid_training_take")
        line_id = int(item.get("line_id", 0))
        text = str(item.get("exact_text", "")).strip()
        wav_file = str(item.get("wav_file", "")).strip()
        if line_id <= 0 or not text or not wav_file or Path(wav_file).name != wav_file:
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

    seen: set[int] = set()
    for item in manifest["held_out_lines"]:
        if not isinstance(item, dict):
            raise VoiceLabProviderError("invalid_held_out_line")
        line_id = int(item.get("line_id", 0))
        text = str(item.get("exact_text", "")).strip()
        if line_id <= 0 or not text or line_id in ids or text in texts or line_id in seen:
            raise VoiceLabProviderError("invalid_held_out_line")
        seen.add(line_id)
    return result

def select_reference(takes: list[dict[str, Any]]) -> dict[str, Any]:
    eligible = [x for x in takes if REFERENCE_MIN_MS <= int(x["duration_ms"]) <= REFERENCE_MAX_MS]
    if not eligible:
        raise VoiceLabProviderError("reference_take_3_to_10_seconds_required")
    return min(
        eligible,
        key=lambda x: (
            abs(int(x["duration_ms"]) - REFERENCE_TARGET_MS),
            int(x["line_id"]),
        ),
    )

def run_stage(source_root: Path, script: Path, env: dict[str, str], *args: str) -> None:
    child_env = os.environ.copy()
    child_env.update(env)
    child_env["PYTHONNOUSERSITE"] = "1"
    child_env["NLTK_DATA"] = str(source_root / "nltk_data")
    runner = Path(__file__).resolve().with_name("voice_lab_upstream_stage.py")
    require_file(runner, "headless_stage_runner")
    result = subprocess.run(
        [sys.executable, "-s", str(runner), str(source_root), str(script), *args],
        cwd=str(source_root),
        env=child_env,
        check=False,
    )
    if result.returncode != 0:
        raise VoiceLabProviderError(f"upstream_stage_failed:{script.name}:{result.returncode}")

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
        return 1, False
    memory_gb = torch.cuda.get_device_properties(0).total_memory / (1024**3) + 0.4
    return max(1, int(memory_gb // 2)), True

def training_configs(
    assets: dict[str, Path], exp: Path, sovits_dir: Path, gpt_dir: Path
) -> tuple[Path, Path, dict[str, str]]:
    import yaml

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
            "if_save_latest": True,
            "if_save_every_weights": True,
            "save_every_epoch": sovits_save_every,
            "gpu_numbers": "0",
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
            "if_save_latest": True,
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
            "_CUDA_VISIBLE_DEVICES": "0",
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
) -> list[dict[str, Any]]:
    sovits_dir = candidate / "_sovits_weights"
    gpt_dir = candidate / "_gpt_weights"
    sovits_dir.mkdir(parents=True, exist_ok=True)
    gpt_dir.mkdir(parents=True, exist_ok=True)
    s2, s1, env = training_configs(assets, exp, sovits_dir, gpt_dir)
    run_stage(source_root, assets["sovits_train"], env, "--config", str(s2))
    run_stage(source_root, assets["gpt_train"], env, "--config_file", str(s1))
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

def evaluate_candidate(
    source_root: Path,
    assets: dict[str, Path],
    candidate: dict[str, Any],
    evaluation_root: Path,
    manifest: dict[str, Any],
    reference: dict[str, Any],
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
            samples.append(
                {
                    "line_id": line_id,
                    "exact_text": held_text,
                    "speaker_similarity": round(score, 6),
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
    return {
        "candidate_id": candidate_id,
        "candidate_order": int(candidate["candidate_order"]),
        "sovits_epoch": int(candidate["sovits_epoch"]),
        "gpt_epoch": int(candidate["gpt_epoch"]),
        "sovits_path": Path(candidate["sovits_path"]),
        "gpt_path": Path(candidate["gpt_path"]),
        "mean_speaker_similarity": round(sum(similarities) / len(similarities), 6),
        "minimum_speaker_similarity": round(min(similarities), 6),
        "samples": samples,
    }

def select_best_candidate(evidence: list[dict[str, Any]]) -> dict[str, Any]:
    if len(evidence) < 2 or len(evidence) > MAX_TRAINING_CANDIDATES:
        raise VoiceLabProviderError(f"candidate_evidence_count_invalid:{len(evidence)}")
    for candidate in evidence:
        samples = candidate.get("samples")
        mean_similarity = candidate.get("mean_speaker_similarity")
        minimum_similarity = candidate.get("minimum_speaker_similarity")
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
    return max(
        evidence,
        key=lambda candidate: (
            float(candidate["mean_speaker_similarity"]),
            float(candidate["minimum_speaker_similarity"]),
            -int(candidate["candidate_order"]),
        ),
    )

def public_candidate_evidence(candidate: dict[str, Any]) -> dict[str, Any]:
    return {
        "candidate_id": str(candidate["candidate_id"]),
        "sovits_epoch": int(candidate["sovits_epoch"]),
        "gpt_epoch": int(candidate["gpt_epoch"]),
        "mean_speaker_similarity": float(candidate["mean_speaker_similarity"]),
        "minimum_speaker_similarity": float(candidate["minimum_speaker_similarity"]),
        "samples": [
            {
                "line_id": int(sample["line_id"]),
                "exact_text": str(sample["exact_text"]),
                "speaker_similarity": float(sample["speaker_similarity"]),
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
            }
        )
    return selected_samples

def build_candidate(
    *,
    source_root: Path,
    dataset_dir: Path,
    candidate_dir: Path,
    evaluation_dir: Path,
    work_dir: Path,
    manifest: dict[str, Any],
    status_writer: Callable[[str, str], None],
) -> None:
    assets = source_assets(source_root)
    takes = training_takes(dataset_dir, manifest)
    reference = select_reference(takes)
    for path in (candidate_dir, evaluation_dir, work_dir):
        if path.exists():
            shutil.rmtree(path)
        path.mkdir(parents=True, exist_ok=True)

    exp = work_dir / "experiment"
    prepare_dataset(source_root, assets, dataset_dir, exp, takes)
    status_writer(
        "training",
        "Creating bounded Voice Actor training candidates from accepted recordings.",
    )
    candidates = train(source_root, assets, exp, candidate_dir)

    status_writer(
        "evaluating",
        "Comparing held-out Voice Actor candidates before review.",
    )
    candidate_evaluation_root = work_dir / "candidate_evaluation"
    candidate_evaluation_root.mkdir(parents=True, exist_ok=True)
    evidence = [
        evaluate_candidate(
            source_root,
            assets,
            candidate,
            candidate_evaluation_root,
            manifest,
            reference,
        )
        for candidate in candidates
    ]
    selected = select_best_candidate(evidence)
    selected_samples = promote_selected_candidate(
        selected,
        candidate_dir,
        evaluation_dir,
        reference,
    )

    selection_method = "held_out_mean_speaker_similarity_then_minimum_tiebreak"
    evaluation_payload = {
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
