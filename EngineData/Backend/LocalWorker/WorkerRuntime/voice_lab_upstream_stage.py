"""Headless execution boundary for pinned GPT-SoVITS VoiceLab stages.

TranslateIT consumes only the approved English VoiceLab / My Voice path. This
module keeps the pinned upstream source intact while removing WebUI and unrelated
multilingual/LoRA import requirements from the product runtime boundary.
"""

from __future__ import annotations

import importlib
import importlib.machinery
import os
import runpy
import sys
import types
from pathlib import Path
from typing import Any

import ffmpeg
import numpy as np


class _EnglishOnlyLangSegmenter:
    @staticmethod
    def getTexts(*_args: Any, **_kwargs: Any) -> list[dict[str, str]]:
        raise RuntimeError("voice_lab:non_english_text_segmentation_not_supported")


def _unsupported_chinese_attribute(name: str) -> Any:
    if name.startswith("__"):
        raise AttributeError(name)
    raise RuntimeError("voice_lab:non_english_text_processing_not_supported")


def _unsupported_lora(*_args: Any, **_kwargs: Any) -> Any:
    raise RuntimeError("voice_lab:unsupported_lora_tts_path")


def clean_path(path_value: Any) -> str:
    path = str(path_value)
    if path.endswith(("\\", "/")):
        return clean_path(path[:-1])
    return path.replace("/", os.sep).replace("\\", os.sep).strip(" '\n\"\u202a")


def _ffmpeg_program(source_root: Path) -> Path:
    return source_root / ("ffmpeg.exe" if os.name == "nt" else "ffmpeg")


def _load_audio(source_root: Path, file: Any, sample_rate: Any) -> np.ndarray:
    audio_path = clean_path(file)
    if not os.path.isfile(audio_path):
        raise RuntimeError("voice_lab:upstream_audio_missing")
    ffmpeg_program = _ffmpeg_program(source_root)
    if not ffmpeg_program.is_file():
        raise RuntimeError("voice_lab:ffmpeg_runtime_missing")
    try:
        output, _ = (
            ffmpeg.input(audio_path, threads=0)
            .output(
                "-",
                format="f32le",
                acodec="pcm_f32le",
                ac=1,
                ar=int(sample_rate),
            )
            .run(
                cmd=[str(ffmpeg_program), "-nostdin"],
                capture_stdout=True,
                capture_stderr=True,
            )
        )
    except Exception as exc:
        raise RuntimeError("voice_lab:upstream_audio_decode_failed") from exc
    return np.frombuffer(output, np.float32).flatten()


def _module(name: str) -> types.ModuleType:
    module = types.ModuleType(name)
    module.__file__ = str(Path(__file__).resolve())
    module.__spec__ = importlib.machinery.ModuleSpec(name, loader=None)
    return module


def _install_english_only_import_shims() -> None:
    text_package = importlib.import_module("text")

    chinese_module = sys.modules.get("text.chinese")
    if chinese_module is None:
        chinese_module = _module("text.chinese")
        chinese_module.__getattr__ = _unsupported_chinese_attribute
        sys.modules["text.chinese"] = chinese_module
    setattr(text_package, "chinese", chinese_module)

    segmenter_module = sys.modules.get("text.LangSegmenter")
    if segmenter_module is None:
        segmenter_module = _module("text.LangSegmenter")
        segmenter_module.LangSegmenter = _EnglishOnlyLangSegmenter
        sys.modules["text.LangSegmenter"] = segmenter_module

    if "peft" not in sys.modules:
        peft_module = _module("peft")
        peft_module.LoraConfig = _unsupported_lora
        peft_module.get_peft_model = _unsupported_lora
        sys.modules["peft"] = peft_module


def _install_english_only_tts_model_init() -> None:
    tts_module = importlib.import_module("TTS_infer_pack.TTS")
    # Use the locked SoundFile decoder; torchaudio 2.11 load requires TorchCodec.
    tts_module.torchaudio = types.SimpleNamespace(
        load=_load_reference_audio,
        transforms=tts_module.torchaudio.transforms,
    )
    tts_class = tts_module.TTS
    if getattr(tts_class, "_translateit_english_only_model_init", False):
        return

    def _init_models(self: Any) -> None:
        self.bert_model = None
        self.bert_tokenizer = None
        self.init_t2s_weights(self.configs.t2s_weights_path)
        self.init_vits_weights(self.configs.vits_weights_path)
        self.init_cnhuhbert_weights(self.configs.cnhuhbert_base_path)

    tts_class._init_models = _init_models
    tts_class._translateit_english_only_model_init = True


def _load_reference_audio(path: Any) -> tuple[Any, int]:
    import soundfile
    import torch

    audio, sample_rate = soundfile.read(path, dtype="float32", always_2d=True)
    return torch.from_numpy(audio.T.copy()), sample_rate


def _run_english_text_stage() -> None:
    from text.cleaner import clean_text

    input_text = Path(str(os.environ.get("inp_text", ""))).resolve()
    output_root = Path(str(os.environ.get("opt_dir", ""))).resolve()
    part = int(os.environ.get("i_part", "0"))
    all_parts = int(os.environ.get("all_parts", "1"))
    version = os.environ.get("version", "v2ProPlus")
    if not input_text.is_file() or all_parts <= 0 or part < 0 or part >= all_parts:
        raise RuntimeError("voice_lab:english_text_stage_input_invalid")

    rows: list[str] = []
    lines = input_text.read_text(encoding="utf-8").strip("\n").split("\n")
    for line in lines[part::all_parts]:
        wav_name, _speaker, language, text = line.split("|", 3)
        if language.strip().lower() != "en":
            raise RuntimeError("voice_lab:non_english_training_text_not_supported")
        phones, word2ph, normalized = clean_text(
            text.replace("%", "-").replace("￥", ","), "en", version
        )
        rows.append(
            f"{os.path.basename(clean_path(wav_name))}\t{' '.join(phones)}\t{word2ph}\t{normalized}"
        )

    output_root.mkdir(parents=True, exist_ok=True)
    target = output_root / f"2-name2text-{part}.txt"
    target.write_text("\n".join(rows) + "\n", encoding="utf-8", newline="\n")


def install_headless_my_utils(source_root: Path) -> None:
    source_root = source_root.resolve()
    gpt_sovits_root = source_root / "GPT_SoVITS"
    if not gpt_sovits_root.is_dir():
        raise RuntimeError("voice_lab:gpt_sovits_source_missing")

    for path in (str(gpt_sovits_root), str(source_root)):
        if path not in sys.path:
            sys.path.insert(0, path)

    _install_english_only_import_shims()

    tools_package = importlib.import_module("tools")
    module = _module("tools.my_utils")
    module.clean_path = clean_path
    module.load_audio = lambda file, sr: _load_audio(source_root, file, sr)
    sys.modules["tools.my_utils"] = module
    setattr(tools_package, "my_utils", module)

    _install_english_only_tts_model_init()


def run_upstream_script(source_root: Path, script: Path, script_args: list[str]) -> None:
    source_root = source_root.resolve()
    script = script.resolve()
    try:
        relative_script = script.relative_to(source_root)
    except ValueError as exc:
        raise RuntimeError("voice_lab:upstream_script_outside_source") from exc
    if not script.is_file():
        raise RuntimeError("voice_lab:upstream_script_missing")

    install_headless_my_utils(source_root)
    if relative_script.as_posix() == "GPT_SoVITS/prepare_datasets/1-get-text.py":
        _run_english_text_stage()
        return

    sys.argv = [str(script), *script_args]
    runpy.run_path(str(script), run_name="__main__")


def main() -> int:
    if len(sys.argv) < 3:
        print(
            "usage: voice_lab_upstream_stage.py <source-root> <script> [args...]",
            file=sys.stderr,
        )
        return 2
    run_upstream_script(Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3:])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
