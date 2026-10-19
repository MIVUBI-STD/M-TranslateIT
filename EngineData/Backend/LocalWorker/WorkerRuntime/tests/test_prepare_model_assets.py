from __future__ import annotations

import importlib.util
from pathlib import Path


def load_module():
    path = Path(__file__).resolve().parents[1] / "prepare_model_assets.py"
    spec = importlib.util.spec_from_file_location("translateit_prepare_model_assets", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_required_huggingface_plan_is_revision_pinned_and_runtime_asset_scoped() -> None:
    module = load_module()
    selected, manual = module.build_plan(module.load_manifest())
    assert {item["model_id"] for item in selected} == {
        "faster-whisper-large-v3-turbo",
        "milmmt-46-1b-v1.0",
    }
    assert all(module.FULL_REVISION.fullmatch(item["revision"]) for item in selected)
    assert all(item["target"].is_relative_to(module.RUNTIME_ASSETS_ROOT) for item in selected)
    assert {item["model_id"] for item in manual} == {"gpt-sovits-v2proplus-voicelab"}


def test_milmmt_plan_pins_exact_revision_and_required_snapshot_files() -> None:
    module = load_module()
    selected, _manual = module.build_plan(module.load_manifest())
    candidate = next(item for item in selected if item["model_id"] == "milmmt-46-1b-v1.0")
    assert candidate["revision"] == "4fc480b6c58dec29c159dcdf9fde0f6d5c354995"
    assert "model.safetensors" in candidate["download_allow_patterns"]
    assert "tokenizer.json" in candidate["download_allow_patterns"]
    assert "tokenizer.model" in candidate["download_allow_patterns"]
    assert "pytorch_model.bin" not in candidate["download_allow_patterns"]


def test_download_allow_pattern_cannot_escape_snapshot() -> None:
    module = load_module()
    try:
        module.validate_download_allow_patterns("example", ["../outside.bin"])
    except RuntimeError as exc:
        assert "may not escape" in str(exc)
    else:
        raise AssertionError("snapshot allow pattern must not escape its model root")


def test_optional_plan_adds_only_manifest_optional_huggingface_assets() -> None:
    module = load_module()
    selected, _manual = module.build_plan(module.load_manifest(), include_optional=True)
    assert {item["model_id"] for item in selected} == {
        "faster-whisper-large-v3-turbo",
        "faster-whisper-medium",
        "milmmt-46-1b-v1.0",
    }


def test_specific_model_selection_rejects_manual_asset() -> None:
    module = load_module()
    try:
        module.build_plan(
            module.load_manifest(),
            requested_ids={"gpt-sovits-v2proplus-voicelab"},
        )
    except RuntimeError as exc:
        assert "not Hugging Face-acquirable" in str(exc)
    else:
        raise AssertionError(
            "manual GPT-SoVITS asset must not be promoted to Hugging Face acquisition"
        )


def test_release_model_stage_uses_only_workspace_without_touching_canonical_targets(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_module()
    monkeypatch.setattr(module._core, "PROJECT_ROOT", tmp_path)
    monkeypatch.setattr(
        module._core, "RUNTIME_ASSETS_ROOT", tmp_path / "EngineData/Backend/RuntimeAssets"
    )
    canonical = module._core.RUNTIME_ASSETS_ROOT / "ASR/ModelData/example"
    items = [{"model_id": "example", "target": canonical, "revision": "a" * 40}]
    stage = tmp_path / ".workspace/staging/release-candidate/payload-tree"
    (stage / "EngineData/Backend").mkdir(parents=True)
    redirected = module._core.stage_download_plan(items, stage)
    assert redirected[0]["target"] == stage / "EngineData/Backend/RuntimeAssets/ASR/ModelData/example"
    assert items[0]["target"] == canonical
    assert not canonical.exists()


def test_release_model_stage_rejects_paths_outside_reserved_workspace(tmp_path: Path, monkeypatch) -> None:
    module = load_module()
    monkeypatch.setattr(module._core, "PROJECT_ROOT", tmp_path)
    monkeypatch.setattr(
        module._core, "RUNTIME_ASSETS_ROOT", tmp_path / "EngineData/Backend/RuntimeAssets"
    )
    model = module._core.RUNTIME_ASSETS_ROOT / "ASR/ModelData/example"
    for forbidden in [tmp_path / "UserData", tmp_path / "EngineData", tmp_path / ".workspace/staging"]:
        (forbidden / "EngineData/Backend").mkdir(parents=True, exist_ok=True)
        try:
            module._core.stage_download_plan([{"target": model}], forbidden)
        except RuntimeError as exc:
            assert "below .workspace/staging" in str(exc)
        else:
            raise AssertionError("Model acquisition must not stage into runtime/user roots")
