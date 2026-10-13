"""Pure unit fixtures for opt-in SoVITS spawned-child strict checkpoint loading.

No GPU, no upstream script, no training, and no monkeypatch of normal runtime.
The tests replace the hard worker exit with a throwing sentinel.
"""

from __future__ import annotations

import hashlib
from pathlib import Path
import tempfile
import unittest

from voice_lab_resume_child_probe import (
    SoVITSChildPlan, StrictChildLoadError, StrictSoVITSChildMonitor,
)


class FakeTensor:
    def __init__(self, shape):
        self.shape = shape


class FakeModel:
    def __init__(self):
        self.weight = FakeTensor((2, 3))
        self.applied = False

    def state_dict(self):
        return {"layer": self.weight}

    def load_state_dict(self, values, strict=False):
        if not strict or set(values) != {"layer"}:
            raise AssertionError("strict model load required")
        self.applied = True


class FakeOptimizer:
    def __init__(self):
        self.loaded = False
        self.state = {
            "state": {}, "param_groups": [{"params": [0]}],
        }

    def state_dict(self):
        return self.state

    def load_state_dict(self, data):
        self.state = data
        self.loaded = True


def sample_data():
    return {
        "iteration": 4,
        "learning_rate": 0.0001,
        "model": {"layer": FakeTensor((2, 3))},
        "optimizer": {
            "state": {0: {"step": 20}},
            "param_groups": [{"params": [0]}],
        },
    }


class SoVITSWorkerChildProbeTests(unittest.TestCase):
    def setUp(self):
        self.work = tempfile.TemporaryDirectory()
        root = Path(self.work.name)
        self.files = {
            "sovits_G": root / "G_233333333333.pth",
            "sovits_D": root / "D_233333333333.pth",
        }
        for role, path in self.files.items():
            path.write_bytes(role.encode())
        def checksum(path):
            return hashlib.sha256(path.read_bytes()).hexdigest()
        self.plan = SoVITSChildPlan(
            run_token="a"*64, recovery_id="b"*64, dataset_id="c"*64,
            g_sha=checksum(self.files["sovits_G"]),
            d_sha=checksum(self.files["sovits_D"]),
            g_path=str(self.files["sovits_G"]), d_path=str(self.files["sovits_D"]),
            expected_epoch=4,
            event_dir=str(root), source_root=str(root),
        )
        self.events = []
        self.exits = []

    def tearDown(self):
        self.work.cleanup()

    def monitor(self, decode=None):
        def fail(code):
            self.exits.append(code)
            raise StrictChildLoadError("hard_abort_would_exit")
        return StrictSoVITSChildMonitor(
            self.plan, pid=1234,
            decode=decode if decode is not None else lambda _: sample_data(),
            emit=self.events.append, hard_abort=fail,
        )

    def test_valid_D_G_load_and_real_first_optimizer_step_signal(self):
        monitor = self.monitor()
        d, gd = FakeModel(), FakeOptimizer()
        g, gg = FakeModel(), FakeOptimizer()
        monitor.load("sovits_D", str(self.files["sovits_D"]), d, gd)
        monitor.load("sovits_G", str(self.files["sovits_G"]), g, gg)
        self.assertTrue(d.applied and g.applied and gd.loaded and gg.loaded)
        self.assertEqual([x["role"] for x in self.events], ["sovits_D", "sovits_G"])
        monitor.mark_epoch(5)
        self.assertFalse(monitor.sent_start)
        monitor.after_first_generator_optimizer_step()
        self.assertTrue(monitor.sent_start)
        self.assertEqual(self.events[2]["kind"], "training_started")
        self.assertEqual(self.events[2]["next_epoch"], 5)
        monitor.after_first_generator_optimizer_step()
        self.assertEqual(len(self.events), 3)
        # Later epochs must proceed without replaying the first-step proof.
        monitor.mark_epoch(6)
        monitor.after_first_generator_optimizer_step()
        self.assertEqual(len(self.events), 3)

    def test_wrong_checkpoint_or_wrong_order_fails_closed(self):
        monitor = self.monitor()
        with self.assertRaisesRegex(StrictChildLoadError, "hard_abort_would_exit"):
            monitor.load("sovits_G", str(self.files["sovits_G"]), FakeModel(), FakeOptimizer())
        self.assertEqual(self.exits, [78])
        self.assertEqual(self.events[-1]["kind"], "fallback")
        self.assertFalse(monitor.loaded)

    def test_invalid_optimizer_or_model_layout_fails_before_success_ack(self):
        for modify in [
            lambda p: p.update(optimizer={"state": {}, "param_groups": [{"params": [0]}]}),
            lambda p: p.update(model={"other": FakeTensor((2,3))}),
            lambda p: p.update(model={"layer": FakeTensor((999,3))}),
            lambda p: p.update(optimizer={"state": {0: {"step": 20}},
                                          "param_groups": [{"params": [0, 1]}]}),
        ]:
            self.events.clear()
            self.exits.clear()
            data = sample_data()
            modify(data)
            monitor = self.monitor(lambda _: data)
            with self.assertRaisesRegex(StrictChildLoadError, "hard_abort_would_exit"):
                monitor.load("sovits_D", str(self.files["sovits_D"]), FakeModel(), FakeOptimizer())
            self.assertEqual([e["kind"] for e in self.events], ["fallback"])
            self.assertEqual(self.exits, [78])

    def test_hash_change_fails_before_deserialization(self):
        monitor = self.monitor(lambda _: self.fail("decoder must not run"))
        self.files["sovits_D"].write_bytes(b"different")
        with self.assertRaisesRegex(StrictChildLoadError, "hard_abort_would_exit"):
            monitor.load("sovits_D", str(self.files["sovits_D"]), FakeModel(), FakeOptimizer())
        self.assertEqual(self.events[-1]["kind"], "fallback")

    def test_epoch_or_missing_actual_step_does_not_ack_resume(self):
        monitor = self.monitor()
        monitor.load("sovits_D", str(self.files["sovits_D"]), FakeModel(), FakeOptimizer())
        monitor.load("sovits_G", str(self.files["sovits_G"]), FakeModel(), FakeOptimizer())
        with self.assertRaisesRegex(StrictChildLoadError, "hard_abort_would_exit"):
            monitor.mark_epoch(6)
        self.assertFalse(any(e["kind"] == "training_started" for e in self.events))

    def test_gd_events_reach_protocol_but_never_become_verified_resume(self):
        from voice_lab_strict_resume_ack import StrictResumeAcknowledgments
        protocol = StrictResumeAcknowledgments(
            self.plan.expectation("0" * 64, gpt_epoch=7, gpt_step=42)
        )
        monitor = self.monitor()
        monitor.load("sovits_D", str(self.files["sovits_D"]),
                     FakeModel(), FakeOptimizer())
        monitor.load("sovits_G", str(self.files["sovits_G"]),
                     FakeModel(), FakeOptimizer())
        monitor.mark_epoch(5)
        monitor.after_first_generator_optimizer_step()
        for event in self.events:
            protocol.observe(event)
        status = protocol.summary()
        self.assertFalse(status["acknowledgment_sequence_complete"])
        self.assertFalse(status["trainer_load_verified"])
        self.assertFalse(status["resume_available"])

    def test_dev_spawn_hook_remains_explicitly_gated_in_normal_stage(self):
        stage_file = (Path(__file__).resolve().parents[1]
                      / "voice_lab_upstream_stage.py")
        stage = stage_file.read_text(encoding="utf-8")
        self.assertIn("--translateit-dev-strict-sovits-probe", stage)
        self.assertIn("len(script_args) != 2", stage)
        self.assertIn("script_args = []", stage)
        build = (Path(__file__).resolve().parents[1]
                 / "voice_lab_gpt_sovits_build.py").read_text(encoding="utf-8")
        self.assertNotIn("--translateit-dev-strict-sovits-probe", build)

    def test_plan_rejects_mismatched_paths_and_epoch(self):
        with self.assertRaisesRegex(StrictChildLoadError, "strict_resume_same_G_D_file"):
            bad = SoVITSChildPlan(**{**vars(self.plan), "d_path": self.plan.g_path})
            bad.validate()
        with self.assertRaisesRegex(StrictChildLoadError, "strict_resume_epoch_not_continuable"):
            bad = SoVITSChildPlan(**{**vars(self.plan), "expected_epoch": 8})
            bad.validate()
        with self.assertRaisesRegex(StrictChildLoadError, "strict_resume_source_root_untrusted"):
            bad = SoVITSChildPlan(**{**vars(self.plan), "source_root": "relative"})
            bad.validate()


if __name__ == "__main__":
    unittest.main()
