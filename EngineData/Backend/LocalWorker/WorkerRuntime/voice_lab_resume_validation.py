"""Developer-only, FAIL-CLOSED audit of interrupted My Voice training archives.

No inference, no training, no subprocess, no writes. No resume action is
exposed. This inspector cannot prove that a trainer actually loaded optimizer
state: a later *runtime acknowledgment* is mandatory.

Default: stdlib-only structural/hash checks. Optional --inspect-tensors uses
torch.load(weights_only=True) on archives from a trusted local source only.
Never deserialize an unknown Internet/imported voice checkpoint automatically.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any, Callable

from voice_lab_gpt_sovits import ENGINE_REVISION, GPT_EPOCHS, SOVITS_EPOCHS

MAX_RECEIPT_BYTES = 16 * 1024
MAX_MANIFEST_BYTES = 512 * 1024
MAX_CHECKPOINT_BYTES = 8 * 1024**3
MAX_PARTS = 28
MAX_GROUP = 12
SUPPORTED_STATUS = "checkpoint_preserved_not_verified_for_resume"
HEX64 = re.compile(r"[a-f0-9]{64}\Z")
PART_LABEL = re.compile(
    r"(?:gpt-(?:[0-9]|1[01])\.ckpt"
    r"|sovits-[GD]-(?:[0-9]|1[01])\.pth"
    r"|sovits-pair-(?:[0-9]|1[01])\.json)\Z"
)
SOURCE_BLOBS = {
    "GPT_SoVITS/s1_train.py": "1176f0bcef869d2f66574d47d9a55d0cc7e1ac0c",
    "GPT_SoVITS/s2_train.py": "333e6a05aedd9f62009ecd6012518116d1326a93",
    "GPT_SoVITS/utils.py": "08e1838420c399ec76266abedf75f9fdf076050b",
}


class ResumeAuditError(ValueError):
    pass


def _regular(path: Path, *, max_bytes: int) -> None:
    if path.is_symlink() or not path.is_file():
        raise ResumeAuditError("missing_or_untrusted_file")
    size = path.stat().st_size
    if not 0 < size <= max_bytes:
        raise ResumeAuditError("invalid_file_size")


def _directory(path: Path) -> None:
    if path.is_symlink() or not path.is_dir():
        raise ResumeAuditError("missing_or_untrusted_directory")


def _hash(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(64 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _json(path: Path, limit: int) -> dict[str, Any]:
    _regular(path, max_bytes=limit)
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (ValueError, UnicodeError) as exc:
        raise ResumeAuditError("invalid_manifest_json") from exc
    if not isinstance(value, dict):
        raise ResumeAuditError("manifest_not_object")
    return value


def _git_blob_sha1(data: bytes) -> str:
    payload = b"blob " + str(len(data)).encode("ascii") + b"\0" + data
    return hashlib.sha1(payload).hexdigest()


def _engine_source(source_root: Path) -> None:
    _directory(source_root)
    _directory(source_root / "GPT_SoVITS")
    marker = source_root / "TRANSLATEIT_GPTSOVITS_REVISION.txt"
    _regular(marker, max_bytes=128)
    if marker.read_text(encoding="utf-8").strip() != ENGINE_REVISION:
        raise ResumeAuditError("engine_revision_mismatch")
    for name, expected in SOURCE_BLOBS.items():
        path = source_root / name
        _regular(path, max_bytes=2 * 1024 * 1024)
        if _git_blob_sha1(path.read_bytes()) != expected:
            raise ResumeAuditError("pinned_trainer_source_mismatch")


def _snapshot(saved_root: Path, snapshot_id: str) -> None:
    if not HEX64.fullmatch(snapshot_id):
        raise ResumeAuditError("dataset_id_invalid")
    directory = saved_root / "DatasetSnapshots"
    _directory(directory)
    path = directory / f"{snapshot_id}.json"
    _regular(path, max_bytes=MAX_MANIFEST_BYTES)
    if _hash(path) != snapshot_id:
        raise ResumeAuditError("dataset_snapshot_hash_mismatch")
    record = _json(path, MAX_MANIFEST_BYTES)
    if record.get("engine_revision") != ENGINE_REVISION:
        raise ResumeAuditError("dataset_engine_mismatch")
    links = record.get("audio")
    takes = (
        record.get("source_manifest", {}).get("takes")
        if isinstance(record.get("source_manifest"), dict)
        else None
    )
    if (
        not isinstance(links, list)
        or not isinstance(takes, list)
        or not 1 <= len(links) <= 128
        or len(links) != len(takes)
    ):
        raise ResumeAuditError("dataset_audio_links_missing_or_mismatched")
    # Read the saved archive's identity and referenced WAV bytes, not arbitrary
    # source paths. This explicit developer audit may stream all selected WAVs.
    for link, take in zip(links, takes):
        if not isinstance(link, dict) or not isinstance(take, dict):
            raise ResumeAuditError("dataset_link_invalid")
        line_id = take.get("line_id")
        transcript = take.get("exact_text")
        wav_name = take.get("wav_file")
        if (
            type(line_id) is not int
            or not 1 <= line_id <= 600
            or not isinstance(transcript, str)
            or not transcript.strip()
            or wav_name != f"take_{line_id:04}.wav"
            or link.get("line_id") != line_id
            or link.get("wav_file") != wav_name
        ):
            raise ResumeAuditError("dataset_link_identity_mismatch")
        archive_id = link.get("archive_id")
        if not isinstance(archive_id, str) or not HEX64.fullmatch(archive_id):
            raise ResumeAuditError("dataset_archive_id_invalid")
        shard = saved_root / "Recordings" / archive_id[:2]
        _directory(shard)
        take_dir = shard / archive_id
        _directory(take_dir)
        take_path = take_dir / "take.json"
        _regular(take_path, max_bytes=16 * 1024)
        if _hash(take_path) != archive_id:
            raise ResumeAuditError("source_take_receipt_hash_mismatch")
        metadata = _json(take_path, 16 * 1024)
        if metadata.get("line_id") != take["line_id"] or metadata.get("exact_text") != take.get(
            "exact_text"
        ):
            raise ResumeAuditError("source_take_transcript_mismatch")
        canonical_hash = metadata.get("canonical_sha256")
        if not isinstance(canonical_hash, str) or not HEX64.fullmatch(canonical_hash):
            raise ResumeAuditError("source_audio_hash_invalid")
        canonical_path = take_dir / "canonical.wav"
        _regular(canonical_path, max_bytes=4 * 1024 * 1024 + 4096)
        if _hash(canonical_path) != canonical_hash:
            raise ResumeAuditError("source_audio_hash_mismatch")


def _checkpoint_progress(original_filename: str, kind: str) -> tuple[int, int]:
    """Sort by source epoch/global_step, NEVER lexicographic archive labels.

    No tensor read, file stat or training involved. Unrecognized legacy
    checkpoint names are not evidence of latest state.
    """
    if kind == "gpt":
        match = re.fullmatch(r"epoch=([0-9]+)-step=([0-9]+)\.ckpt", original_filename)
        if match is None:
            raise ResumeAuditError("gpt_checkpoint_progress_unverifiable")
        epoch, step = int(match.group(1)), int(match.group(2))
        if epoch < 0 or step < 1:
            raise ResumeAuditError("gpt_checkpoint_progress_invalid")
        return epoch, step
    if kind in ("G", "D"):
        match = re.fullmatch(rf"{kind}_([0-9]+)\.pth", original_filename)
        if match is None:
            raise ResumeAuditError("sovits_checkpoint_progress_unverifiable")
        step = int(match.group(1))
        if step <= 0:
            raise ResumeAuditError("sovits_checkpoint_progress_invalid")
        return 0, step
    raise ResumeAuditError("checkpoint_role_unknown")


def audit_archive(
    saved_root: Path,
    recovery_id: str,
    *,
    source_root: Path | None = None,
    inspect_tensors: bool = False,
    tensor_loader: Callable[[Path], Any] | None = None,
) -> dict[str, Any]:
    """Read only. Returns blocking reasons; never returns resume_available=True."""
    _directory(saved_root)
    if not HEX64.fullmatch(recovery_id):
        raise ResumeAuditError("recovery_id_invalid")
    directory = saved_root / "TrainingRecovery" / recovery_id
    _directory(saved_root / "TrainingRecovery")
    _directory(directory)
    receipt_path = directory / "recovery.json"
    receipt = _json(receipt_path, MAX_RECEIPT_BYTES)
    if _hash(receipt_path) != recovery_id:
        raise ResumeAuditError("recovery_receipt_hash_mismatch")
    if (
        receipt.get("format") != 1
        or receipt.get("engine_revision") != ENGINE_REVISION
        or receipt.get("status") != SUPPORTED_STATUS
    ):
        raise ResumeAuditError("unsupported_recovery_contract")
    snapshot = receipt.get("dataset_snapshot_id")
    if not isinstance(snapshot, str):
        raise ResumeAuditError("dataset_id_missing")
    _snapshot(saved_root, snapshot)

    parts = receipt.get("parts")
    if not isinstance(parts, list) or not 1 <= len(parts) <= MAX_PARTS:
        raise ResumeAuditError("recovery_parts_invalid")
    folder = directory / "files"
    _directory(folder)
    labels: dict[str, Path] = {}
    originals: dict[str, str] = {}
    for part in parts:
        if not isinstance(part, dict):
            raise ResumeAuditError("part_not_object")
        label, sha, length, original = (
            part.get("label"),
            part.get("sha256"),
            part.get("bytes"),
            part.get("original_filename"),
        )
        if not (
            isinstance(label, str)
            and (PART_LABEL.fullmatch(label) or label in ("gpt-config.yaml", "sovits-config.json"))
        ):
            raise ResumeAuditError("part_label_invalid")
        if label in labels:
            raise ResumeAuditError("duplicate_part")
        if not (
            isinstance(sha, str)
            and HEX64.fullmatch(sha)
            and type(length) is int
            and 0 < length <= MAX_CHECKPOINT_BYTES
            and isinstance(original, str)
            and 1 <= len(original) <= 128
            and re.fullmatch(r"[A-Za-z0-9_.=-]+", original)
        ):
            raise ResumeAuditError("part_metadata_invalid")
        path = folder / label
        _regular(path, max_bytes=MAX_CHECKPOINT_BYTES)
        if path.stat().st_size != length or _hash(path) != sha:
            raise ResumeAuditError("checkpoint_bytes_changed")
        labels[label] = path
        originals[label] = original

    blockers: list[str] = []
    tensor_inspected = False
    gpts = [p for p in labels if re.fullmatch(r"gpt-(?:[0-9]|1[01])\.ckpt", p)]
    gens = [p for p in labels if re.fullmatch(r"sovits-G-(?:[0-9]|1[01])\.pth", p)]
    discs = [p for p in labels if re.fullmatch(r"sovits-D-(?:[0-9]|1[01])\.pth", p)]
    # "gpt-10" sorts before "gpt-9" lexicographically. For optional
    # tensor inspection, choose the actual latest source epoch/step and G/D
    # global step; unreadable progress metadata blocks inspection.
    for family, group in (("gpt", gpts), ("G", gens), ("D", discs)):
        try:
            group.sort(key=lambda label: _checkpoint_progress(originals[label], family))
        except ResumeAuditError as exc:
            blockers.append(str(exc))
    pairs = sorted(p for p in labels if re.fullmatch(r"sovits-pair-(?:[0-9]|1[01])\.json", p))
    if any(len(group) > MAX_GROUP for group in (gpts, gens, discs, pairs)):
        raise ResumeAuditError("recovery_checkpoint_group_count_exceeded")
    if not gpts:
        blockers.append("gpt_full_checkpoint_missing")
    if not gens or not discs:
        blockers.append("sovits_generator_discriminator_pair_missing")
    if {p.split("-")[2] for p in gens} != {p.split("-")[2] for p in discs}:
        blockers.append("sovits_checkpoint_pair_index_mismatch")
    for g_label in gens:
        d_label = g_label.replace("-G-", "-D-")
        if d_label not in originals:
            continue
        g_name, d_name = originals[g_label], originals[d_label]
        if (
            not re.fullmatch(r"G_\d+\.pth", g_name)
            or not re.fullmatch(r"D_\d+\.pth", d_name)
            or g_name[2:] != d_name[2:]
        ):
            blockers.append("sovits_original_checkpoint_pair_mismatch")
    for label in gpts:
        if not originals[label].endswith(".ckpt"):
            blockers.append("gpt_original_checkpoint_name_invalid")
    # Resume inventory must not confuse a hash-valid archive with matching
    # original G/D tensor checkpoint evidence. Validate tiny pair receipts
    # against already verified per-file hashes; do NOT rehash GB weights.
    paired_steps: set[int] = set()
    for label in pairs:
        receipt = _json(labels[label], 4096)
        original = originals[label]
        match = re.fullmatch(r"pair_([0-9]+)\.json", original)
        if not match or type(receipt.get("global_step")) is not int:
            raise ResumeAuditError("sovits_pair_receipt_identity_invalid")
        step = int(match.group(1))
        if (
            receipt.get("format") != 1
            or step < 1
            or receipt["global_step"] != step
            or type(receipt.get("epoch")) is not int
            or not 1 <= receipt["epoch"] <= SOVITS_EPOCHS
        ):
            raise ResumeAuditError("sovits_pair_receipt_schema_invalid")
        for role, key in (("G", "generator"), ("D", "discriminator")):
            member = receipt.get(key)
            matching = [
                item
                for item in (gens if role == "G" else discs)
                if originals[item] == f"{role}_{step}.pth"
            ]
            if len(matching) != 1 or not isinstance(member, dict):
                raise ResumeAuditError("sovits_pair_receipt_member_missing")
            component = matching[0]
            # Recovery source receipt hashes are already checked above.
            record = next(p for p in parts if p["label"] == component)
            if (
                member.get("name") != originals[component]
                or member.get("sha256") != record["sha256"]
                or member.get("bytes") != record["bytes"]
            ):
                raise ResumeAuditError("sovits_pair_receipt_hash_mismatch")
        if step in paired_steps:
            raise ResumeAuditError("sovits_pair_duplicate_step")
        paired_steps.add(step)
    if gens and discs and not pairs:
        blockers.append("sovits_paired_checkpoint_receipt_absent_legacy")
    if not {"gpt-config.yaml", "sovits-config.json"} <= labels.keys():
        blockers.append("training_config_missing")
    if source_root is None:
        blockers.append("pinned_trainer_files_not_verified")
    else:
        _engine_source(source_root)

    if inspect_tensors and not blockers:
        try:
            configs = _configs(labels)
            if tensor_loader is None:
                import torch

                # No fallback to unsafe weights_only=False, even if an
                # otherwise legitimate Lightning checkpoint is rejected.
                def strict_tensor_loader(path: Path):
                    return torch.load(path, map_location="cpu", weights_only=True)

                tensor_loader = strict_tensor_loader
            parsed: dict[str, Any] = {
                label: tensor_loader(labels[label]) for label in (gpts[-1], gens[-1], discs[-1])
            }
        except Exception:
            # A safe-deserialization limitation is not proof that preserved
            # bytes are corrupt. Keep integrity distinct from resume readiness.
            blockers.append("checkpoint_tensor_inspection_unavailable")
        else:
            tensor_inspected = True
            blockers.extend(
                _training_state_blockers(
                    parsed[gpts[-1]],
                    parsed[gens[-1]],
                    parsed[discs[-1]],
                    configs,
                )
            )
    elif not inspect_tensors:
        blockers.append("optimizer_state_not_inspected")

    # Structural verification is NOT an observed upstream resume.
    blockers.append("trainer_resume_load_acknowledgment_not_implemented")
    blockers.append("sovits_scaler_scheduler_continuity_unproven")
    return {
        "format": 1,
        "checkpoint_id": recovery_id,
        "dataset_snapshot_id": snapshot,
        "file_count": len(parts),
        "integrity_verified": True,
        "tensor_structure_inspected": tensor_inspected,
        "resume_available": False,
        "blockers": sorted(set(blockers)),
    }


def _configs(labels: dict[str, Path]) -> tuple[dict[str, Any], dict[str, Any]]:
    try:
        import yaml
    except ImportError as exc:
        raise ResumeAuditError("yaml_unavailable_for_explicit_inspection") from exc
    try:
        s1 = yaml.safe_load(labels["gpt-config.yaml"].read_text(encoding="utf-8"))
        s2 = _json(labels["sovits-config.json"], MAX_MANIFEST_BYTES)
    except (ValueError, UnicodeError, yaml.YAMLError) as exc:
        raise ResumeAuditError("training_configuration_invalid") from exc
    if not isinstance(s1, dict):
        raise ResumeAuditError("gpt_configuration_invalid")
    return s1, s2


def _has_optimizer(value: Any) -> bool:
    return (
        isinstance(value, dict)
        and isinstance(value.get("state"), dict)
        and bool(value["state"])
        and isinstance(value.get("param_groups"), list)
        and bool(value["param_groups"])
    )


def _training_state_blockers(
    gpt: Any,
    g: Any,
    d: Any,
    configs: tuple[dict[str, Any], dict[str, Any]],
) -> list[str]:
    blockers = []
    s1, s2 = configs
    s1_train = s1.get("train") if isinstance(s1.get("train"), dict) else {}
    s2_train = s2.get("train") if isinstance(s2.get("train"), dict) else {}
    s2_model = s2.get("model") if isinstance(s2.get("model"), dict) else {}
    if (
        s1_train.get("epochs") != GPT_EPOCHS
        or s2_train.get("epochs") != SOVITS_EPOCHS
        or s1_train.get("precision") not in ("16-mixed", "32")
        or s2_train.get("fp16_run") not in (True, False)
        or type(s1_train.get("if_save_latest")) is not bool
        or type(s2_train.get("if_save_latest")) is not bool
        or s2_model.get("version") != "v2ProPlus"
    ):
        blockers.append("training_configuration_mismatch")
    if (
        not isinstance(gpt, dict)
        or not isinstance(gpt.get("state_dict"), dict)
        or not gpt["state_dict"]
    ):
        blockers.append("gpt_model_state_missing")
    if not isinstance(gpt, dict) or type(gpt.get("epoch")) is not int or gpt["epoch"] < 0:
        blockers.append("gpt_epoch_invalid")
    if (
        not isinstance(gpt, dict)
        or type(gpt.get("global_step")) is not int
        or gpt["global_step"] <= 0
    ):
        blockers.append("gpt_global_step_invalid")
    if (
        not isinstance(gpt, dict)
        or not isinstance(gpt.get("optimizer_states"), list)
        or not any(_has_optimizer(state) for state in gpt["optimizer_states"])
    ):
        blockers.append("gpt_optimizer_state_missing")
    if (
        not isinstance(gpt, dict)
        or not isinstance(gpt.get("lr_schedulers"), list)
        or not gpt["lr_schedulers"]
    ):
        blockers.append("gpt_scheduler_state_missing")
    iterations = []
    for label, value in (("generator", g), ("discriminator", d)):
        if (
            not isinstance(value, dict)
            or not isinstance(value.get("model"), dict)
            or not value["model"]
        ):
            blockers.append(f"sovits_{label}_model_missing")
        if not isinstance(value, dict) or not _has_optimizer(value.get("optimizer")):
            blockers.append(f"sovits_{label}_optimizer_missing")
        epoch = value.get("iteration") if isinstance(value, dict) else None
        if type(epoch) is not int or not 1 <= epoch <= SOVITS_EPOCHS:
            blockers.append(f"sovits_{label}_iteration_invalid")
        else:
            iterations.append(epoch)
    if len(iterations) == 2 and iterations[0] != iterations[1]:
        blockers.append("sovits_generator_discriminator_epoch_mismatch")
    if s2_train.get("fp16_run") is True:
        blockers.append("sovits_amp_scaler_state_absent_upstream")
    return blockers


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--saved-root", type=Path, required=True)
    parser.add_argument("--recovery-id", required=True)
    parser.add_argument("--source-root", type=Path)
    parser.add_argument(
        "--inspect-tensors",
        action="store_true",
        help="Explicit trusted-file-only safe deserialization; never automatic",
    )
    args = parser.parse_args()
    try:
        report = audit_archive(
            args.saved_root,
            args.recovery_id,
            source_root=args.source_root,
            inspect_tensors=args.inspect_tensors,
        )
    except ResumeAuditError as exc:
        print(
            json.dumps(
                {"integrity_verified": False, "resume_available": False, "blockers": [str(exc)]},
                sort_keys=True,
            )
        )
        return 2
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
