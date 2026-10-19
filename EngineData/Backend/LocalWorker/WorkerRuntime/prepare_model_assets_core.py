from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import uuid
from pathlib import Path
from typing import Any

from huggingface_hub import snapshot_download

MANIFEST_PATH = Path(__file__).with_name("model_manifest.json")
PROJECT_ROOT = Path(__file__).resolve().parents[4]
RUNTIME_ASSETS_ROOT = (PROJECT_ROOT / "EngineData/Backend/RuntimeAssets").resolve()
FULL_REVISION = re.compile(r"^[0-9a-f]{40}$")


def load_manifest() -> dict[str, Any]:
    data = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    if data.get("schema") != "translateit.local_model_inventory.v2":
        raise RuntimeError("model_manifest.json has an unsupported schema")
    if data.get("inventory_scope") != "full_product_release_assets":
        raise RuntimeError("model_manifest.json has an unexpected inventory scope")
    models = data.get("models")
    if not isinstance(models, list) or not models:
        raise RuntimeError("model_manifest.json contains no model entries")
    return data


def resolve_target(expected_path: str) -> Path:
    raw = Path(expected_path)
    if raw.is_absolute():
        raise RuntimeError(f"Model expected_path must be repository-relative: {expected_path}")
    target = (PROJECT_ROOT / raw).resolve()
    try:
        target.relative_to(RUNTIME_ASSETS_ROOT)
    except ValueError as exc:
        raise RuntimeError(
            f"Model expected_path escapes the canonical RuntimeAssets root: {expected_path}"
        ) from exc
    return target


def validate_download_allow_patterns(model_id: str, value: Any) -> list[str] | None:
    if value in (None, []):
        return None
    if not isinstance(value, list) or not value:
        raise RuntimeError(f"{model_id} download_allow_patterns must be a non-empty list")

    patterns: list[str] = []
    for raw in value:
        pattern = str(raw or "").strip().replace("\\", "/")
        if not pattern or pattern.startswith("/"):
            raise RuntimeError(f"{model_id} has invalid download allow pattern: {raw!r}")
        parts = [part for part in pattern.split("/") if part]
        if any(part == ".." for part in parts):
            raise RuntimeError(f"{model_id} download allow pattern may not escape its snapshot")
        patterns.append(pattern)
    return list(dict.fromkeys(patterns))


def validate_huggingface_model(model: dict[str, Any]) -> dict[str, Any]:
    model_id = str(model.get("model_id", "")).strip()
    repo_id = str(model.get("repo_id", "")).strip()
    revision = str(model.get("revision", "")).strip()
    expected_path = str(model.get("expected_path", "")).strip()
    if not model_id or not repo_id or not expected_path:
        raise RuntimeError(f"Incomplete Hugging Face model entry: {model_id or '<missing-id>'}")
    if model.get("install_method") != "huggingface_snapshot_download":
        raise RuntimeError(f"Unexpected install method for {model_id}")
    if not FULL_REVISION.fullmatch(revision):
        raise RuntimeError(f"{model_id} must pin a full Hugging Face commit revision")
    target = resolve_target(expected_path)
    return {
        "model_id": model_id,
        "repo_id": repo_id,
        "revision": revision,
        "required": bool(model.get("required")),
        "stage": str(model.get("stage", "")),
        "target": target,
        "download_allow_patterns": validate_download_allow_patterns(
            model_id, model.get("download_allow_patterns")
        ),
    }


def build_plan(
    manifest: dict[str, Any],
    include_optional: bool = False,
    requested_ids: set[str] | None = None,
) -> tuple[list[dict[str, Any]], list[dict[str, str]]]:
    selected: list[dict[str, Any]] = []
    manual_required: list[dict[str, str]] = []
    seen_ids: set[str] = set()
    seen_targets: set[Path] = set()

    for raw in manifest["models"]:
        model_id = str(raw.get("model_id", "")).strip()
        if not model_id:
            raise RuntimeError("Model manifest contains an entry without model_id")
        if model_id in seen_ids:
            raise RuntimeError(f"Duplicate model_id in manifest: {model_id}")
        seen_ids.add(model_id)

        source_type = str(raw.get("source_type", "")).strip()
        if source_type == "huggingface":
            item = validate_huggingface_model(raw)
            target = item["target"]
            if target in seen_targets:
                raise RuntimeError(f"Duplicate model target in manifest: {target}")
            seen_targets.add(target)
            if requested_ids is not None:
                if model_id in requested_ids:
                    selected.append(item)
            elif item["required"] or include_optional:
                selected.append(item)
        elif bool(raw.get("required")):
            manual_required.append(
                {
                    "model_id": model_id,
                    "stage": str(raw.get("stage", "")),
                    "install_method": str(raw.get("install_method", "manual")),
                }
            )

    if requested_ids is not None:
        missing = requested_ids - seen_ids
        if missing:
            raise RuntimeError(f"Unknown model_id requested: {', '.join(sorted(missing))}")
        unsupported = requested_ids - {item["model_id"] for item in selected}
        if unsupported:
            raise RuntimeError(
                "Requested model is not Hugging Face-acquirable: " + ", ".join(sorted(unsupported))
            )

    if not selected:
        raise RuntimeError("No Hugging Face model assets selected")
    return selected, manual_required


def clean_huggingface_local_cache(directory: Path) -> None:
    cache = directory / ".cache" / "huggingface"
    if cache.exists():
        shutil.rmtree(cache)
    cache_parent = directory / ".cache"
    if cache_parent.exists() and not any(cache_parent.iterdir()):
        cache_parent.rmdir()


def replace_target_from_snapshot(item: dict[str, Any]) -> dict[str, Any]:
    """Admit a checked snapshot without destroying an interrupted rollback.

    The final marker is part of the same staged directory as the model bytes.
    Never reuse a PID-derived path: interrupted backups must not be deleted
    merely because a new Python process happens to get the same PID.
    """
    target: Path = item["target"]
    if target.is_symlink():
        raise RuntimeError("Model target cannot be a filesystem symlink")
    target.parent.mkdir(parents=True, exist_ok=True)
    transaction = uuid.uuid4().hex
    staging = target.parent / f".{target.name}.download-{transaction}"
    backup = target.parent / f".{target.name}.previous-{transaction}"

    had_previous = target.exists()
    moved_previous = False
    promoted = False
    try:
        download_args: dict[str, Any] = {
            "repo_id": item["repo_id"],
            "revision": item["revision"],
            "local_dir": staging,
        }
        if item.get("download_allow_patterns"):
            download_args["allow_patterns"] = item["download_allow_patterns"]
        snapshot_download(**download_args)
        clean_huggingface_local_cache(staging)
        files = [path for path in staging.rglob("*") if path.is_file()]
        if not files:
            raise RuntimeError(f"Downloaded snapshot for {item['model_id']} contains no files")
        # The marker must be committed along with the snapshot, not written
        # after replacing the live model. A crash cannot publish an unmarked
        # model while claiming that the previous version was verified.
        (staging / ".translateit_model_revision").write_text(
            str(item["revision"]).strip() + "\n", encoding="utf-8"
        )
        if had_previous:
            target.replace(backup)
            moved_previous = True
        try:
            staging.replace(target)
            promoted = True
        except Exception:
            if moved_previous and backup.exists() and not target.exists():
                backup.replace(target)
                moved_previous = False
            raise
        # Only retire the old version after the new directory and revision
        # marker have both been moved into the final canonical location.
        if moved_previous:
            shutil.rmtree(backup)
        return {
            "model_id": item["model_id"],
            "repo_id": item["repo_id"],
            "revision": item["revision"],
            "file_count": len(files),
            "replaced_existing": had_previous,
        }
    finally:
        if promoted:
            # Cleanup only this successfully committed transaction.
            if staging.exists():
                shutil.rmtree(staging)
        # On failed download/promotion, preserve unverified staged files and
        # especially any backup that could be required for manual recovery.
        # The next transaction obtains a fresh UUID and cannot overwrite them.


def stage_download_plan(
    items: list[dict[str, Any]], destination_root: Path
) -> list[dict[str, Any]]:
    """Redirect only vetted manifest targets into a private release staging tree."""
    root = destination_root.resolve()
    project_root = PROJECT_ROOT.resolve()
    if any((PROJECT_ROOT / name).is_symlink() for name in (".workspace",)):
        raise RuntimeError("Release staging cannot use a redirected workspace root")
    safe_parent = (PROJECT_ROOT / ".workspace" / "staging").resolve()
    if not safe_parent.is_relative_to(project_root):
        raise RuntimeError("Release staging cannot leave the repository volume")
    if root == safe_parent or not root.is_relative_to(safe_parent):
        raise RuntimeError("Release model staging must stay below .workspace/staging")
    if not (root / "EngineData" / "Backend").is_dir():
        raise RuntimeError("Release staging Backend root must already exist")
    redirected: list[dict[str, Any]] = []
    for item in items:
        relative = item["target"].relative_to(RUNTIME_ASSETS_ROOT)
        target = root / "EngineData" / "Backend" / "RuntimeAssets" / relative
        redirected.append({**item, "target": target})
    return redirected


def serializable_plan(items: list[dict[str, Any]]) -> list[dict[str, Any]]:
    output: list[dict[str, Any]] = []
    for item in items:
        entry = {
            "model_id": item["model_id"],
            "repo_id": item["repo_id"],
            "revision": item["revision"],
            "required": item["required"],
            "stage": item["stage"],
            "expected_path": str(item["target"].relative_to(PROJECT_ROOT)).replace("\\", "/"),
        }
        if item.get("download_allow_patterns"):
            entry["download_allow_patterns"] = item["download_allow_patterns"]
        output.append(entry)
    return output


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Acquire TranslateIT developer Hugging Face model assets from pinned manifest revisions."
    )
    parser.add_argument(
        "--include-optional",
        action="store_true",
        help="Also acquire optional Hugging Face entries such as the ASR fallback.",
    )
    parser.add_argument(
        "--model-id",
        action="append",
        default=[],
        help="Acquire only a specific manifest model_id. May be repeated.",
    )
    parser.add_argument(
        "--destination-root",
        type=Path,
        help="Release-only isolated model target beneath .workspace/staging.",
    )
    parser.add_argument(
        "--plan",
        action="store_true",
        help="Validate and print the pinned acquisition plan without downloading files.",
    )
    args = parser.parse_args()

    requested = set(args.model_id) if args.model_id else None
    manifest = load_manifest()
    selected, manual_required = build_plan(manifest, args.include_optional, requested)
    if args.destination_root is not None:
        selected = stage_download_plan(selected, args.destination_root)
    output: dict[str, Any] = {
        "schema": "translateit.developer_model_acquisition.v1",
        "manifest_schema": manifest["schema"],
        "plan_only": bool(args.plan),
        "selected": serializable_plan(selected),
        "manual_required_assets": manual_required,
        "downloads": [],
        "note": (
            "Pinned Hugging Face assets are acquired from model_manifest.json. Required release assets "
            "such as the pinned GPT-SoVITS VoiceLab bundle remain separate and are not fabricated by this developer downloader."
        ),
    }

    if not args.plan:
        output["downloads"] = [replace_target_from_snapshot(item) for item in selected]

    print(json.dumps(output, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
