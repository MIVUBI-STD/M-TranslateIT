"""No model/GPU needed: fail-closed resume admission contract fixtures."""

from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from voice_lab_gpt_sovits import ENGINE_REVISION, GPT_EPOCHS, SOVITS_EPOCHS
from voice_lab_resume_validation import (
    ResumeAuditError,
    _training_state_blockers,
    _checkpoint_progress,
    audit_archive,
)


def save_json(path: Path, data: dict) -> str:
    payload = json.dumps(data, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(payload)
    return hashlib.sha256(payload).hexdigest()


def fixture(
    root: Path, *, omit_discriminator: bool = False,
    paired: bool = False, incorrect_pair_sha: bool = False,
) -> str:
    saved = root / "VoiceLab"
    audio = b"RIFF" + b"\0" * 100
    digest = hashlib.sha256(audio).hexdigest()
    receipt = {
        "schema_version": 1,
        "line_id": 1,
        "exact_text": "Good morning, everyone.",
        "canonical_sha256": digest,
        "native_rate_sha256": None,
    }
    take_id = save_json(root / "temp_take.json", receipt)
    take_dir = saved / "Recordings" / take_id[:2] / take_id
    save_json(take_dir / "take.json", receipt)
    (take_dir / "canonical.wav").write_bytes(audio)
    snapshot = {
        "schema_version": 1, "engine_revision": ENGINE_REVISION,
        "source_manifest": {
            "schema_version": 1, "authorized_voice_confirmed": True,
            "takes": [{"line_id": 1, "exact_text": "Good morning, everyone.", "wav_file": "take_0001.wav"}],
            "held_out_lines": [],
        },
        "audio": [{"line_id": 1, "wav_file": "take_0001.wav", "archive_id": take_id}],
        "from_saved_archive": True,
    }
    snapshot_path = saved / "DatasetSnapshots" / "unused.json"
    snapshot_id = save_json(snapshot_path, snapshot)
    (snapshot_path.parent / f"{snapshot_id}.json").write_bytes(snapshot_path.read_bytes())
    snapshot_path.unlink()

    checkpoint_dir = saved / "TrainingRecovery"
    archive_parts = [
        ("gpt-0.ckpt", "epoch=7-step=42.ckpt", b"lightning full state"),
        ("sovits-G-0.pth", "G_233333333333.pth", b"generator optimizer state"),
        ("gpt-config.yaml", "translateit_s1.yaml", b"train: {epochs: 15}\n"),
        ("sovits-config.json", "translateit_s2.json", b'{"train":{"epochs":8,"fp16_run":true}}'),
    ]
    if not omit_discriminator:
        archive_parts.append(("sovits-D-0.pth", "D_233333333333.pth", b"discriminator state"))
    if paired:
        g_bytes, d_bytes = b"G" * 8192, b"D" * 8192
        archive_parts = [
            (name, "G_120.pth", g_bytes) if name == "sovits-G-0.pth"
            else (name, "D_120.pth", d_bytes) if name == "sovits-D-0.pth"
            else (name, original, content)
            for name, original, content in archive_parts
        ]
        pair_receipt = {
            "format": 1, "epoch": 2, "global_step": 120,
            "generator": {"name": "G_120.pth", "bytes": len(g_bytes),
                          "sha256": hashlib.sha256(g_bytes).hexdigest()},
            "discriminator": {"name": "D_120.pth", "bytes": len(d_bytes),
                              "sha256": ("0" * 64 if incorrect_pair_sha
                                         else hashlib.sha256(d_bytes).hexdigest())},
        }
        pair_raw = json.dumps(pair_receipt, separators=(",", ":"), sort_keys=True).encode()
        archive_parts.append(("sovits-pair-0.json", "pair_120.json", pair_raw))
    archive_parts.sort(key=lambda part: part[0])
    parts = [
        {
            "label": name, "original_filename": original,
            "sha256": hashlib.sha256(content).hexdigest(),
            "bytes": len(content),
        }
        for name, original, content in archive_parts
    ]
    receipt = {
        "format": 1,
        "engine_revision": ENGINE_REVISION,
        "dataset_snapshot_id": snapshot_id,
        "status": "checkpoint_preserved_not_verified_for_resume",
        "parts": parts,
    }
    temp = checkpoint_dir / "receipt.temp"
    ident = save_json(temp, receipt)
    dest = checkpoint_dir / ident
    save_json(dest / "recovery.json", receipt)
    for name, _, content in archive_parts:
        path = dest / "files" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
    temp.unlink()
    return ident


class RecoveryAuditContractTests(unittest.TestCase):
    def test_progress_order_uses_original_epoch_and_step(self) -> None:
        names = [
            "epoch=9-step=200.ckpt",
            "epoch=10-step=150.ckpt",
            "epoch=10-step=300.ckpt",
        ]
        self.assertEqual(
            sorted(names, key=lambda name: _checkpoint_progress(name, "gpt"))[-1],
            "epoch=10-step=300.ckpt",
        )
        self.assertGreater(
            _checkpoint_progress("G_1200.pth", "G"),
            _checkpoint_progress("G_900.pth", "G"),
        )
        with self.assertRaisesRegex(ResumeAuditError, "gpt_checkpoint_progress_unverifiable"):
            _checkpoint_progress("last.ckpt", "gpt")

    def test_hash_valid_archive_still_does_not_enable_resume(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            saved = Path(tmp) / "VoiceLab"
            name = fixture(Path(tmp))
            report = audit_archive(saved, name)
            self.assertTrue(report["integrity_verified"])
            self.assertFalse(report["resume_available"])
            self.assertIn("optimizer_state_not_inspected", report["blockers"])
            self.assertIn("trainer_resume_load_acknowledgment_not_implemented", report["blockers"])
            self.assertFalse(report["tensor_structure_inspected"])

    def test_new_paired_archive_is_auditable_but_still_not_resumable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            saved = Path(tmp) / "VoiceLab"
            name = fixture(Path(tmp), paired=True)
            report = audit_archive(saved, name)
            self.assertTrue(report["integrity_verified"])
            self.assertFalse(report["resume_available"])
            self.assertNotIn("sovits_paired_checkpoint_receipt_absent_legacy", report["blockers"])

    def test_inconsistent_pair_digest_is_rejected_not_resumed(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            saved = Path(tmp) / "VoiceLab"
            name = fixture(Path(tmp), paired=True, incorrect_pair_sha=True)
            with self.assertRaisesRegex(ResumeAuditError, "sovits_pair_receipt_hash_mismatch"):
                audit_archive(saved, name)

    def test_mutated_checkpoint_fails_without_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            saved = Path(tmp) / "VoiceLab"
            name = fixture(Path(tmp))
            file = saved / "TrainingRecovery" / name / "files" / "gpt-0.ckpt"
            file.write_bytes(b"changed")
            with self.assertRaisesRegex(ResumeAuditError, "checkpoint_bytes_changed"):
                audit_archive(saved, name)

    def test_missing_discriminator_is_not_ready(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            saved = Path(tmp) / "VoiceLab"
            name = fixture(Path(tmp), omit_discriminator=True)
            report = audit_archive(saved, name)
            self.assertIn("sovits_generator_discriminator_pair_missing", report["blockers"])
            self.assertFalse(report["resume_available"])

    def test_untrusted_recovery_id_rejected_without_path_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            saved = Path(tmp) / "VoiceLab"
            name = fixture(Path(tmp))
            with self.assertRaisesRegex(ResumeAuditError, "recovery_id_invalid"):
                audit_archive(saved, "../" + name)

    def test_gpt_and_sovits_optimizer_states_and_epochs_must_match(self) -> None:
        optimizer = {"state": {0: {"step": 10}}, "param_groups": [{"lr": 0.0001}]}
        gpt = {
            "state_dict": {"layer": object()},
            "epoch": 7, "global_step": 42,
            "optimizer_states": [optimizer],
            "lr_schedulers": [{"last_epoch": 7}],
        }
        g = {"model": {"net": object()}, "iteration": 3,
             "learning_rate": 0.0001, "optimizer": optimizer}
        d = {"model": {"net": object()}, "iteration": 4,
             "learning_rate": 0.0001, "optimizer": optimizer}
        cfg = ({"train": {"epochs": GPT_EPOCHS, "precision": "16-mixed",
                          "if_save_latest": False}},
               {"train": {"epochs": SOVITS_EPOCHS, "fp16_run": True,
                          "if_save_latest": False},
                "model": {"version": "v2ProPlus"}})
        issues = _training_state_blockers(gpt, g, d, cfg)
        self.assertIn("sovits_generator_discriminator_epoch_mismatch", issues)
        self.assertIn("sovits_amp_scaler_state_absent_upstream", issues)
        self.assertNotIn("gpt_optimizer_state_missing", issues)
        d["iteration"] = 3
        cfg[1]["train"]["fp16_run"] = False
        self.assertEqual(_training_state_blockers(gpt, g, d, cfg), [])

    def test_legacy_checkpoint_config_is_recognized_but_not_resume_proven(self) -> None:
        gpt = {"state_dict": {"x": 1}, "epoch": 3, "global_step": 4,
               "optimizer_states": [{"state": {1: "x"}, "param_groups": [{"lr": 0.01}]}],
               "lr_schedulers": [{"step": 2}]}
        sov = {"model": {"x": 1}, "iteration": 2,
               "optimizer": {"state": {1: "x"}, "param_groups": [{"lr": 0.01}]}}
        config = ({"train": {"epochs": GPT_EPOCHS, "precision": "32",
                             "if_save_latest": True}},
                  {"train": {"epochs": SOVITS_EPOCHS, "fp16_run": False,
                             "if_save_latest": True},
                   "model": {"version": "v2ProPlus"}})
        self.assertNotIn("training_configuration_mismatch",
                         _training_state_blockers(gpt, sov, sov, config))

    def test_missing_optimizer_state_is_not_obscured(self) -> None:
        cfg = ({"train": {"epochs": GPT_EPOCHS, "precision": "32",
                           "if_save_latest": True}},
               {"train": {"epochs": SOVITS_EPOCHS, "fp16_run": False,
                          "if_save_latest": True},
                "model": {"version": "v2ProPlus"}})
        issues = _training_state_blockers({}, {}, {}, cfg)
        self.assertIn("gpt_optimizer_state_missing", issues)
        self.assertIn("sovits_generator_optimizer_missing", issues)
        self.assertIn("sovits_discriminator_optimizer_missing", issues)


if __name__ == "__main__":
    unittest.main()


def test_updated_checkpoint_counts_and_legacy_resume_remains_disabled():
    from voice_lab_resume_validation import MAX_PARTS, PART_LABEL
    assert MAX_PARTS == 28
    assert PART_LABEL.fullmatch("sovits-pair-11.json")
    assert PART_LABEL.fullmatch("gpt-11.ckpt")
    assert not PART_LABEL.fullmatch("gpt-12.ckpt")
