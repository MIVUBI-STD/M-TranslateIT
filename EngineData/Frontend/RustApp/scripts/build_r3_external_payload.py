#!/usr/bin/env python3
"""Build TranslateIT's version-bound R3 external offline payload.

Build-time only: no runtime download/bootstrap behavior. The script packages the
already staged private runtime into one 7z/LZMA2 archive, validates that Windows
bsdtar can read it, binds the payload to the application/dependency/model identity,
and renders the trusted NSIS hook consumed by the Tauri build.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

PAYLOAD_SCHEMA = "translateit.r3.external_payload.v1"
INSTALLED_RUNTIME_SCHEMA = "translateit.installed_runtime.v1"
PAYLOAD_FILENAME = "TranslateIT-Payload.7z"
PAYLOAD_CONTRACT = (
    "EngineData/Backend/LocalWorker/PythonRuntime/TRANSLATEIT_PAYLOAD_CONTRACT.json"
)
PYTHON_VERSION = "3.12.10"
TORCH_VERSION = "2.11.0"
TRANSFORMERS_VERSION = "4.57.6"
TOKENIZERS_VERSION = "0.22.2"
MILMMT_REVISION = "4fc480b6c58dec29c159dcdf9fde0f6d5c354995"
ASR_REVISION = "0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf"
GPT_SOVITS_REVISION = "d523079fc05d9a8028d6085bffe4a2757c32abb6"

PAYLOAD_ROOTS = (
    "EngineData/Backend/LocalWorker/PythonRuntime",
    "EngineData/Backend/RuntimeAssets/ASR/ModelData",
    "EngineData/Backend/RuntimeAssets/Translation/ModelData",
    "EngineData/Backend/RuntimeAssets/Voice/BuiltInVoices",
    "EngineData/Backend/RuntimeAssets/Voice/GPTSoVITS",
    "EngineData/Backend/RuntimeAssets/AudioProvider/VBCABLE/Package",
)
REQUIRED_FILES = {
    "EngineData/Backend/LocalWorker/PythonRuntime/python.exe": None,
    "EngineData/Backend/RuntimeAssets/ASR/ModelData/faster-whisper-large-v3-turbo/.translateit_model_revision": ASR_REVISION,
    "EngineData/Backend/RuntimeAssets/Translation/ModelData/xiaomi-research--MiLMMT-46-1B-v1.0/.translateit_model_revision": MILMMT_REVISION,
    "EngineData/Backend/RuntimeAssets/Voice/BuiltInVoices/MaleVoice/reference.wav": None,
    "EngineData/Backend/RuntimeAssets/Voice/BuiltInVoices/MaleVoice/REFERENCE_SOURCE.txt": None,
    "EngineData/Backend/RuntimeAssets/Voice/BuiltInVoices/FemaleVoice/reference.wav": None,
    "EngineData/Backend/RuntimeAssets/Voice/BuiltInVoices/FemaleVoice/REFERENCE_SOURCE.txt": None,
    "EngineData/Backend/RuntimeAssets/Voice/BuiltInVoices/SOURCES.json": None,
    "EngineData/Backend/RuntimeAssets/Voice/GPTSoVITS/Source/TRANSLATEIT_GPTSOVITS_REVISION.txt": GPT_SOVITS_REVISION,
    "EngineData/Backend/RuntimeAssets/AudioProvider/VBCABLE/Package/VBCABLE_Setup_x64.exe": None,
    "EngineData/Backend/RuntimeAssets/AudioProvider/VBCABLE/Package/VBCABLE_Setup.exe": None,
}


def require_file(path: Path, label: str) -> None:
    if not path.is_file() or path.stat().st_size <= 0:
        raise RuntimeError(f"missing required file: {label}: {path}")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(8 * 1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_bytes(root: Path) -> int:
    return sum(path.stat().st_size for path in root.rglob("*") if path.is_file())


def direct_child_bytes(root: Path, limit: int = 20) -> dict[str, int]:
    rows: list[tuple[str, int]] = []
    for path in root.iterdir():
        size = tree_bytes(path) if path.is_dir() else path.stat().st_size
        rows.append((path.name, size))
    rows.sort(key=lambda item: (-item[1], item[0].lower()))
    return dict(rows[:limit])


def app_version(repo_root: Path) -> str:
    app_root = repo_root / "EngineData/Frontend/RustApp"
    package = json.loads((app_root / "package.json").read_text(encoding="utf-8"))
    tauri = json.loads((app_root / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
    package_version = str(package.get("version") or "").strip()
    tauri_version = str(tauri.get("version") or "").strip()
    if not package_version or package_version != tauri_version:
        raise RuntimeError(
            f"application version drift: package.json={package_version!r}, tauri.conf.json={tauri_version!r}"
        )
    return package_version


def validate_staged_inputs(repo_root: Path) -> None:
    for relative in PAYLOAD_ROOTS:
        if not (repo_root / relative).is_dir():
            raise RuntimeError(f"missing payload root: {relative}")
    for relative, expected in REQUIRED_FILES.items():
        path = repo_root / relative
        require_file(path, relative)
        if expected is not None and path.read_text(encoding="utf-8").strip() != expected:
            raise RuntimeError(f"payload marker mismatch: {relative}")


def is_filesystem_indirection(path: Path) -> bool:
    if path.is_symlink():
        return True
    is_junction = getattr(path, "is_junction", None)
    return bool(is_junction and is_junction())


def validate_no_filesystem_indirection(repo_root: Path) -> None:
    for relative in PAYLOAD_ROOTS:
        root = repo_root / relative
        if is_filesystem_indirection(root):
            raise RuntimeError(f"payload filesystem indirection is forbidden: {relative}")
        for path in root.rglob("*"):
            if is_filesystem_indirection(path):
                display = path.relative_to(repo_root).as_posix()
                raise RuntimeError(f"payload filesystem indirection is forbidden: {display}")


def write_payload_contract(repo_root: Path, version: str) -> Path:
    path = repo_root / PAYLOAD_CONTRACT
    path.parent.mkdir(parents=True, exist_ok=True)
    body = {
        "schema": PAYLOAD_SCHEMA,
        "app_version": version,
        "payload_file": PAYLOAD_FILENAME,
        "python_version": PYTHON_VERSION,
        "torch_version": TORCH_VERSION,
        "transformers_version": TRANSFORMERS_VERSION,
        "tokenizers_version": TOKENIZERS_VERSION,
        "revisions": {
            "milmmt": MILMMT_REVISION,
            "faster_whisper": ASR_REVISION,
            "gpt_sovits": GPT_SOVITS_REVISION,
        },
        "vb_cable": {
            "setup_owned_install": True,
            "install_args": ["-i", "-h"],
            "windows_driver_consent_may_appear": True,
            "restart_required_after_new_install": True,
            "uninstall_policy": "preserve_system_driver",
        },
        "user_data_policy": "preserve_app_local_user_data",
        "network_download": False,
        "manual_extraction": False,
        "second_user_facing_installer": False,
    }
    path.write_text(json.dumps(body, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def find_7zip() -> str:
    candidates: list[str] = []
    configured = os.environ.get("TRANSLATEIT_7ZIP", "").strip()
    if configured:
        candidates.append(configured)
    for command in ("7z.exe", "7zz.exe", "7z", "7zz"):
        resolved = shutil.which(command)
        if resolved:
            candidates.append(resolved)
    if os.name == "nt":
        candidates.extend(
            [r"C:\Program Files\7-Zip\7z.exe", r"C:\Program Files (x86)\7-Zip\7z.exe"]
        )
    for candidate in candidates:
        path = Path(candidate)
        if path.is_file():
            return str(path)
        resolved = shutil.which(candidate)
        if resolved:
            return resolved
    raise RuntimeError(
        "7-Zip CLI is required on the release build machine only. Install 7-Zip or set "
        "TRANSLATEIT_7ZIP; normal users do not need 7-Zip."
    )


def find_tar() -> str:
    for candidate in ("tar.exe", "tar"):
        resolved = shutil.which(candidate)
        if resolved:
            return resolved
    raise RuntimeError("Windows tar/bsdtar is required to validate the generated R3 payload.")


def create_payload(repo_root: Path, output: Path, seven_zip: str, tar_program: str) -> list[str]:
    output.parent.mkdir(parents=True, exist_ok=True)
    output.unlink(missing_ok=True)
    subprocess.run(
        [
            seven_zip,
            "a",
            "-t7z",
            "-m0=LZMA2",
            "-mx=9",
            "-ms=on",
            "-mmt=on",
            "-y",
            str(output),
            *PAYLOAD_ROOTS,
        ],
        cwd=repo_root,
        check=True,
    )
    require_file(output, PAYLOAD_FILENAME)

    listing = subprocess.run(
        [tar_program, "-tf", str(output)],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    ).stdout.splitlines()
    entries = [line.strip().replace("\\", "/") for line in listing if line.strip()]
    if not entries:
        raise RuntimeError("payload archive listing is empty")

    exact_roots = set(PAYLOAD_ROOTS)
    prefixes = tuple(root.rstrip("/") + "/" for root in PAYLOAD_ROOTS)
    normalized_entries = {entry.rstrip("/") for entry in entries}
    for entry in entries:
        normalized = entry.rstrip("/")
        if normalized.startswith("/") or "../" in f"/{normalized}/":
            raise RuntimeError(f"unsafe payload archive entry: {entry}")
        if normalized not in exact_roots and not normalized.startswith(prefixes):
            raise RuntimeError(f"payload entry escapes approved roots: {entry}")
    for required in (*REQUIRED_FILES, PAYLOAD_CONTRACT):
        if required.replace("\\", "/") not in normalized_entries:
            raise RuntimeError(f"required payload entry absent from archive: {required}")
    return entries


def render_hook(
    template: Path,
    output: Path,
    helper: Path,
    version: str,
    digest: str,
    expanded_bytes: int,
) -> None:
    rendered = template.read_text(encoding="utf-8")
    replacements = {
        "@@PAYLOAD_SCHEMA@@": PAYLOAD_SCHEMA,
        "@@INSTALLED_RUNTIME_SCHEMA@@": INSTALLED_RUNTIME_SCHEMA,
        "@@APP_VERSION@@": version,
        "@@PAYLOAD_FILENAME@@": PAYLOAD_FILENAME,
        "@@PAYLOAD_SHA256@@": digest,
        "@@PAYLOAD_EXPANDED_BYTES@@": str(expanded_bytes),
        "@@INSTALLER_HELPER_SOURCE@@": str(helper.resolve()).replace("/", "\\"),
    }
    for token, value in replacements.items():
        if token not in rendered:
            raise RuntimeError(f"installer hook template token missing: {token}")
        rendered = rendered.replace(token, value)
    if "@@" in rendered:
        raise RuntimeError("installer hook contains unresolved placeholders")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(rendered, encoding="utf-8", newline="\n")


def write_evidence(
    path: Path,
    payload: Path,
    version: str,
    digest: str,
    expanded_bytes: int,
    root_bytes: dict[str, int],
    python_runtime_top_level_bytes: dict[str, int],
    torch_top_level_bytes: dict[str, int],
    torch_lib_largest_files: dict[str, int],
    entries: list[str],
    seven_zip: str,
    tar_program: str,
) -> None:
    body = {
        "schema": PAYLOAD_SCHEMA,
        "installed_runtime_schema": INSTALLED_RUNTIME_SCHEMA,
        "app_version": version,
        "payload_file": PAYLOAD_FILENAME,
        "payload_sha256": digest,
        "payload_bytes": payload.stat().st_size,
        "expanded_bytes": expanded_bytes,
        "compression_ratio": round(payload.stat().st_size / expanded_bytes, 6)
        if expanded_bytes
        else None,
        "payload_root_bytes": root_bytes,
        "python_runtime_top_level_bytes": python_runtime_top_level_bytes,
        "torch_top_level_bytes": torch_top_level_bytes,
        "torch_lib_largest_files": torch_lib_largest_files,
        "archive_format": "7z",
        "compression": "LZMA2",
        "compression_level": 9,
        "build_7zip_program": seven_zip,
        "target_extract_program": "Windows tar.exe / bsdtar",
        "validation_tar_program": tar_program,
        "entry_count": len(entries),
        "payload_roots": list(PAYLOAD_ROOTS),
        "user_facing_files": ["TranslateIT-Setup.exe", PAYLOAD_FILENAME],
        "installer_mode": "perMachine",
        "network_download": False,
        "manual_extraction": False,
        "second_user_facing_installer": False,
        "target_pc_acceptance": "deferred",
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(body, indent=2) + "\n", encoding="utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--hook-template", required=True, type=Path)
    parser.add_argument("--installer-helper", required=True, type=Path)
    parser.add_argument("--generated-hook", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = args.repo_root.resolve()
    if not (repo_root / "AGENTS.md").is_file():
        raise RuntimeError(f"invalid TranslateIT repository root: {repo_root}")
    require_file(args.hook_template, "R3 NSIS hook template")
    require_file(args.installer_helper, "R3 installer helper")

    version = app_version(repo_root)
    validate_staged_inputs(repo_root)
    validate_no_filesystem_indirection(repo_root)
    write_payload_contract(repo_root, version)
    root_bytes = {root: tree_bytes(repo_root / root) for root in PAYLOAD_ROOTS}
    python_runtime_root = repo_root / "EngineData/Backend/LocalWorker/PythonRuntime"
    python_runtime_top_level_bytes = direct_child_bytes(python_runtime_root)
    torch_root = python_runtime_root / "torch"
    torch_top_level_bytes = direct_child_bytes(torch_root) if torch_root.is_dir() else {}
    torch_lib_root = torch_root / "lib"
    torch_lib_largest_files = (
        direct_child_bytes(torch_lib_root, limit=30) if torch_lib_root.is_dir() else {}
    )
    expanded_bytes = sum(root_bytes.values())
    seven_zip = find_7zip()
    tar_program = find_tar()
    payload = args.output.resolve()
    entries = create_payload(repo_root, payload, seven_zip, tar_program)
    digest = sha256_file(payload)
    render_hook(
        args.hook_template.resolve(),
        args.generated_hook.resolve(),
        args.installer_helper.resolve(),
        version,
        digest,
        expanded_bytes,
    )
    write_evidence(
        args.evidence.resolve(),
        payload,
        version,
        digest,
        expanded_bytes,
        root_bytes,
        python_runtime_top_level_bytes,
        torch_top_level_bytes,
        torch_lib_largest_files,
        entries,
        seven_zip,
        tar_program,
    )
    print(f"[r3-payload] app_version={version}")
    print(f"[r3-payload] payload={payload}")
    print(f"[r3-payload] sha256={digest}")
    print(f"[r3-payload] expanded_bytes={expanded_bytes}")
    print(
        "[r3-payload] payload_root_bytes="
        + json.dumps(root_bytes, sort_keys=True, separators=(",", ":"))
    )
    print(
        "[r3-payload] python_runtime_top_level_bytes="
        + json.dumps(python_runtime_top_level_bytes, sort_keys=False, separators=(",", ":"))
    )
    print(
        f"[r3-payload] compression_ratio="
        f"{(payload.stat().st_size / expanded_bytes):.6f}"
        if expanded_bytes
        else "[r3-payload] compression_ratio=unavailable"
    )
    print(f"[r3-payload] entries={len(entries)}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:  # build boundary must fail closed with one clear reason
        print(f"[r3-payload] FAILED: {exc}", file=sys.stderr)
        raise SystemExit(1)
