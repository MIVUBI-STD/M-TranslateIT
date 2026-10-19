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


def test_failed_model_download_preserves_existing_and_interrupted_backups(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_module()
    target = tmp_path / "model"
    target.mkdir()
    (target / "old.bin").write_bytes(b"accepted")
    # This simulates a still-unverified backup from a previous interrupted run.
    previous = tmp_path / ".model.previous-previous-process"
    previous.mkdir()
    (previous / "recover.bin").write_bytes(b"important")
    def fail_download(**kwargs) -> None:
        kwargs["local_dir"].mkdir()
        (kwargs["local_dir"] / "partial.bin").write_bytes(b"incomplete")
        raise RuntimeError("controlled download failure")
    monkeypatch.setattr(module._core, "snapshot_download", fail_download)
    item = {
        "target": target, "model_id": "test", "repo_id": "offline/example",
        "revision": "a" * 40,
    }
    try:
        module._core.replace_target_from_snapshot(item)
    except RuntimeError as exc:
        assert "controlled download failure" in str(exc)
    else:
        raise AssertionError("Failed model download must not report success")
    assert (target / "old.bin").read_bytes() == b"accepted"
    assert (previous / "recover.bin").read_bytes() == b"important"
    assert list(tmp_path.glob(".model.download-*")), "Failed temporary model remains for review"


def test_promoted_snapshot_has_revision_marker_before_directory_cutover(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_module()
    target = tmp_path / "model"
    target.mkdir()
    (target / "old.bin").write_bytes(b"old")
    def populate(**kwargs) -> None:
        staged = kwargs["local_dir"]
        staged.mkdir()
        (staged / "model.bin").write_bytes(b"new")
    monkeypatch.setattr(module._core, "snapshot_download", populate)
    item = {
        "target": target, "model_id": "test", "repo_id": "offline/example",
        "revision": "b" * 40,
    }
    result = module._core.replace_target_from_snapshot(item)
    assert result["replaced_existing"] is True
    assert (target / "model.bin").read_bytes() == b"new"
    assert (target / ".translateit_model_revision").read_text(encoding="utf-8") == "b" * 40 + "\n"
    assert not list(tmp_path.glob(".model.previous-*"))


def test_failed_promotion_restores_previous_model_without_deleting_backups(
    tmp_path: Path, monkeypatch
) -> None:
    module = load_module()
    target = tmp_path / "model"
    target.mkdir()
    (target / "original.bin").write_bytes(b"original")
    def populate(**kwargs) -> None:
        staged = kwargs["local_dir"]
        staged.mkdir()
        (staged / "next.bin").write_bytes(b"next")
    monkeypatch.setattr(module._core, "snapshot_download", populate)
    original_replace = Path.replace
    def fail_new_model_cutover(self: Path, destination: Path) -> Path:
        if self.name.startswith(".model.download-"):
            raise OSError("simulated stage cutover failure")
        return original_replace(self, destination)
    monkeypatch.setattr(Path, "replace", fail_new_model_cutover)
    item = {
        "target": target, "model_id": "test", "repo_id": "offline/example",
        "revision": "c" * 40,
    }
    try:
        module._core.replace_target_from_snapshot(item)
    except OSError as exc:
        assert "stage cutover failure" in str(exc)
    else:
        raise AssertionError("A failed model promotion must not pass")
    assert (target / "original.bin").read_bytes() == b"original"
    assert not (target / "next.bin").exists()
