"""Pure GPT worker witness fixtures. No Lightning, GPU or model execution."""

from __future__ import annotations

import hashlib
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from voice_lab_resume_gpt_probe import GptProbeError, GptWorkerPlan, GptWorkerWitness, _same_state


def sha(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


class FakeModule:
    def state_dict(self):
        return {"layer.a": 1, "layer.b": 2}


class FakeOptimizer:
    def __init__(self):
        self.state = {
            "state": {0: {"step": 11}},
            "param_groups": [{"params": [0]}],
        }

    def state_dict(self):
        return self.state


class FakeScheduler:
    def state_dict(self):
        return {"last_epoch": 5, "gamma": 0.99}


class FakeConfig:
    def __init__(self):
        self.scheduler = FakeScheduler()


class FakeTrainer:
    def __init__(self):
        self.global_step = 42
        self.current_epoch = 7
        self.optimizers = [FakeOptimizer()]
        self.lr_scheduler_configs = [FakeConfig()]


class GptWorkerProbeTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.checkpoint = root / "epoch=7-step=42.ckpt"
        self.checkpoint.write_bytes(b"fixture GPT Lightning checkpoint")
        self.config = root / "config.yaml"
        self.config.write_bytes(b"train: {epochs: 15}\n")
        self.plan = GptWorkerPlan(
            run_token="a" * 64,
            recovery_id="b" * 64,
            dataset_id="c" * 64,
            source_root=str(root),
            checkpoint_path=str(self.checkpoint),
            checkpoint_sha256=sha(self.checkpoint.read_bytes()),
            config_path=str(self.config),
            config_sha256=sha(self.config.read_bytes()),
            expected_epoch=7,
            expected_global_step=42,
            event_dir=str(root),
        )
        self.events = []
        self.aborts = []

    def tearDown(self):
        self.tmp.cleanup()

    def payload(self):
        return {
            "epoch": 7,
            "global_step": 42,
            "state_dict": FakeModule().state_dict(),
            "optimizer_states": [FakeOptimizer().state_dict()],
            "lr_schedulers": [FakeScheduler().state_dict()],
        }

    def witness(self, payload=None):
        def fail(code):
            self.aborts.append(code)
            raise GptProbeError("hard_abort_fixture")

        return GptWorkerWitness(
            self.plan,
            pid=1234,
            emit=self.events.append,
            abort=fail,
            decode=lambda _: payload if payload is not None else self.payload(),
        )

    def test_explicit_checkpoint_load_then_real_step_and_global_step(self):
        witness = self.witness()
        checkpoint = witness.preflight(self.checkpoint)
        trainer = FakeTrainer()
        witness.restored(trainer, FakeModule(), checkpoint)
        self.assertEqual([x["kind"] for x in self.events], ["checkpoint_loaded"])
        self.assertFalse(witness.started)
        witness.after_batch(trainer)
        self.assertFalse(witness.started)
        witness.after_optimizer_step()
        # Merely returning from optimizer.step without trainer progress is not enough.
        with self.assertRaisesRegex(GptProbeError, "hard_abort_fixture"):
            witness.after_batch(trainer)
        self.assertEqual(self.aborts, [78])
        self.assertEqual(self.events[-1]["kind"], "fallback")

    def test_real_step_and_incremented_global_step_emits_training_start(self):
        witness = self.witness()
        trainer = FakeTrainer()
        saved = witness.preflight(self.checkpoint)
        witness.restored(trainer, FakeModule(), saved)
        witness.after_optimizer_step()
        trainer.global_step += 1
        witness.after_batch(trainer)
        self.assertTrue(witness.started)
        self.assertEqual(
            [x["kind"] for x in self.events], ["checkpoint_loaded", "training_started"]
        )
        self.assertEqual(self.events[1]["global_step"], 42)
        self.assertEqual(self.events[1]["observed_global_step"], 43)
        witness.after_batch(trainer)
        self.assertEqual(len(self.events), 2)
        witness.finished()

    def test_missing_ckpt_path_never_falls_back_to_pretrained(self):
        witness = self.witness()
        with self.assertRaisesRegex(GptProbeError, "hard_abort_fixture"):
            witness.preflight(None)
        self.assertEqual(self.aborts, [78])
        self.assertEqual(self.events[0]["kind"], "fallback")

    def test_checksum_mismatch_aborts_before_checkpoint_decode(self):
        witness = self.witness({"not": "used"})
        self.checkpoint.write_bytes(b"changed checkpoint")
        with self.assertRaisesRegex(GptProbeError, "hard_abort_fixture"):
            witness.preflight(self.checkpoint)
        self.assertEqual(self.aborts, [78])

    def test_optimizer_scheduler_model_or_global_step_mismatch(self):
        edits = [
            lambda x: x.update(optimizer_states=[]),
            lambda x: x.update(lr_schedulers=[]),
            lambda x: x.update(state_dict={"wrong": 123}),
            lambda x: x.update(global_step=99),
        ]
        for edit in edits:
            self.events.clear()
            self.aborts.clear()
            payload = self.payload()
            edit(payload)
            witness = self.witness(payload)
            with self.assertRaisesRegex(GptProbeError, "hard_abort_fixture"):
                saved = witness.preflight(self.checkpoint)
                witness.restored(FakeTrainer(), FakeModule(), saved)
            self.assertEqual(self.aborts, [78])
            self.assertEqual(self.events[-1]["kind"], "fallback")

    def test_live_optimizer_scheduler_and_model_must_equal_checkpoint(self):
        for kind in ("model", "optimizer", "scheduler", "global_step"):
            self.events.clear()
            self.aborts.clear()
            witness = self.witness()
            saved = witness.preflight(self.checkpoint)
            trainer, model = FakeTrainer(), FakeModule()
            if kind == "model":
                model.state_dict = lambda: {"layer.a": 3, "layer.b": 2}
            elif kind == "optimizer":
                trainer.optimizers[0].state["state"][0]["step"] = 12
            elif kind == "scheduler":
                trainer.lr_scheduler_configs[0].scheduler.state_dict = lambda: {
                    "last_epoch": 6,
                    "gamma": 0.99,
                }
            else:
                trainer.global_step = 4
            with self.assertRaisesRegex(GptProbeError, "hard_abort_fixture"):
                witness.restored(trainer, model, saved)
            self.assertEqual([x["kind"] for x in self.events], ["fallback"])

    def test_requires_complete_evidence_before_success(self):
        witness = self.witness()
        checkpoint = witness.preflight(self.checkpoint)
        witness.restored(FakeTrainer(), FakeModule(), checkpoint)
        with self.assertRaisesRegex(GptProbeError, "hard_abort_fixture"):
            witness.finished()
        self.assertFalse(witness.started)

    def test_only_matching_absolute_trusted_plan(self):
        self.plan.validate()
        for field, replacement in [
            ("checkpoint_sha256", "bad"),
            ("config_path", "relative/path"),
            ("expected_epoch", 15),
            ("expected_global_step", 0),
        ]:
            with self.assertRaises(GptProbeError):
                replace(self.plan, **{field: replacement}).validate()

    def test_nested_optimizer_equality_is_bounded_and_strict(self):
        self.assertTrue(_same_state({"a": [1, 2]}, {"a": [1, 2]}))
        self.assertFalse(_same_state({"a": [1, 2]}, {"a": [1, 3]}))
        self.assertFalse(_same_state({"a": [1, 2]}, {"a": [1, 2]}, budget=[0]))

    def test_dev_probe_opt_in_never_used_by_normal_build(self):
        root = Path(__file__).resolve().parents[1]
        stage = (root / "voice_lab_upstream_stage.py").read_text(encoding="utf-8")
        build = (root / "voice_lab_gpt_sovits_build.py").read_text(encoding="utf-8")
        self.assertIn("--translateit-dev-strict-gpt-probe", stage)
        self.assertIn('relative_script.as_posix() != "GPT_SoVITS/s1_train.py"', stage)
        self.assertIn('script_args = ["--config_file", plan.config_path]', stage)
        self.assertNotIn("--translateit-dev-strict-gpt-probe", build)


if __name__ == "__main__":
    unittest.main()
