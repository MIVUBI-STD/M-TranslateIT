"""Pure acknowledgment-protocol regression fixtures; no worker/GPU runs."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from voice_lab_strict_resume_ack import (
    ResumeAckError,
    ResumeExpectation,
    StrictResumeAcknowledgments,
    verify_worker_event_log,
)

TOKEN = "a" * 64
RECOVERY = "b" * 64
DATASET = "c" * 64
GPT_SHA = "d" * 64
GEN_SHA = "e" * 64
DISC_SHA = "f" * 64


def expectation() -> ResumeExpectation:
    return ResumeExpectation(
        run_token=TOKEN, recovery_id=RECOVERY, dataset_id=DATASET,
        checkpoint_sha256={"gpt": GPT_SHA, "sovits_G": GEN_SHA, "sovits_D": DISC_SHA},
        gpt_epoch=7, gpt_global_step=400, sovits_epoch=4,
    )


def event(number: int, kind: str, role: str, *, pid: int = 410, **extra) -> dict:
    return {
        "schema_version": 1, "run_token": TOKEN,
        "recovery_id": RECOVERY, "dataset_id": DATASET,
        "engine_revision": expectation().engine_revision,
        "worker_pid": pid, "sequence": number,
        "kind": kind, "role": role, **extra,
    }


def full_trace() -> list[dict]:
    return [
        event(1, "checkpoint_loaded", "sovits_D",
              checkpoint_sha256=DISC_SHA, model_loaded=True, optimizer_loaded=True,
              checkpoint_epoch=4),
        event(2, "checkpoint_loaded", "sovits_G",
              checkpoint_sha256=GEN_SHA, model_loaded=True, optimizer_loaded=True,
              checkpoint_epoch=4),
        event(3, "training_started", "sovits", next_epoch=5),
        event(4, "checkpoint_loaded", "gpt", pid=412,
              checkpoint_sha256=GPT_SHA, model_loaded=True, optimizer_loaded=True,
              scheduler_loaded=True, checkpoint_epoch=7, global_step=400),
        event(5, "training_started", "gpt", pid=412,
              current_epoch=7, global_step=400),
    ]


class StrictWorkerAcknowledgmentContractTests(unittest.TestCase):
    def test_complete_simulated_trace_is_never_native_resume_ready(self):
        session = StrictResumeAcknowledgments(expectation())
        for item in full_trace():
            session.observe(item)
        result = session.summary()
        self.assertTrue(result["acknowledgment_sequence_complete"])
        self.assertFalse(result["worker_provenance_verified"])
        self.assertFalse(result["trainer_load_verified"])
        self.assertFalse(result["resume_available"])
        self.assertIn("native_resume_reproducibility_not_tested", result["blockers"])

    def test_partial_trace_cannot_claim_checkpoint_loaded(self):
        session = StrictResumeAcknowledgments(expectation())
        session.observe(full_trace()[0])
        self.assertFalse(session.summary()["acknowledgment_sequence_complete"])

    def test_missing_discriminator_prevents_train_start(self):
        session = StrictResumeAcknowledgments(expectation())
        trace = full_trace()
        with self.assertRaisesRegex(ResumeAckError, "sovits_training_started_without_checkpoint_pair"):
            session.observe(trace[0])
            session.observe({**trace[2], "sequence": 2})
        self.assertFalse(session.summary()["acknowledgment_sequence_complete"])

    def test_fallback_is_sticky_failure_even_after_valid_events(self):
        session = StrictResumeAcknowledgments(expectation())
        session.observe(full_trace()[0])
        with self.assertRaisesRegex(ResumeAckError, "trainer_fell_back_to_pretrained_weights"):
            session.observe(event(2, "fallback", "sovits"))
        with self.assertRaisesRegex(ResumeAckError, "session_already_failed"):
            session.observe(full_trace()[1])
        self.assertFalse(session.summary()["acknowledgment_sequence_complete"])

    def test_mismatched_worker_or_optimizer_or_checkpoint_rejected(self):
        for alteration, error in [
            ({"optimizer_loaded": False}, "model_or_optimizer_load_unconfirmed"),
            ({"checkpoint_sha256": "0" * 64}, "loaded_checkpoint_identity_mismatch"),
            ({"checkpoint_epoch": 5}, "checkpoint_epoch_mismatch"),
        ]:
            session = StrictResumeAcknowledgments(expectation())
            with self.assertRaisesRegex(ResumeAckError, error):
                session.observe({**full_trace()[0], **alteration})
        session = StrictResumeAcknowledgments(expectation())
        session.observe(full_trace()[0])
        with self.assertRaisesRegex(ResumeAckError, "sovits_pair_different_worker"):
            session.observe({**full_trace()[1], "worker_pid": 999})

    def test_gpt_scheduler_and_true_loaded_global_step_required(self):
        session = StrictResumeAcknowledgments(expectation())
        with self.assertRaisesRegex(ResumeAckError, "gpt_scheduler_load_unconfirmed"):
            session.observe({**full_trace()[3], "sequence": 1, "scheduler_loaded": False})
        session = StrictResumeAcknowledgments(expectation())
        with self.assertRaisesRegex(ResumeAckError, "gpt_checkpoint_global_step_mismatch"):
            session.observe({**full_trace()[3], "sequence": 1, "global_step": 1})

    def test_duplicate_and_out_of_order_events_poison_session(self):
        session = StrictResumeAcknowledgments(expectation())
        with self.assertRaisesRegex(ResumeAckError, "acknowledgment_sequence_invalid"):
            session.observe(full_trace()[1])
        session = StrictResumeAcknowledgments(expectation())
        session.observe(full_trace()[0])
        with self.assertRaisesRegex(ResumeAckError, "checkpoint_role_missing_or_duplicate"):
            session.observe({**full_trace()[0], "sequence": 2})

    def test_read_only_bounded_jsonl_verifier(self):
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "worker-ack.jsonl"
            path.write_text("".join(json.dumps(item) + "\n" for item in full_trace()),
                            encoding="utf-8")
            original = path.read_bytes()
            result = verify_worker_event_log(path, expectation())
            self.assertTrue(result["acknowledgment_sequence_complete"])
            self.assertFalse(result["resume_available"])
            self.assertEqual(path.read_bytes(), original)
            path.write_text("{invalid-json}\n", encoding="utf-8")
            with self.assertRaises(ResumeAckError):
                verify_worker_event_log(path, expectation())

    def test_invalid_expectation_never_admitted(self):
        with self.assertRaisesRegex(ResumeAckError, "run_token_invalid"):
            StrictResumeAcknowledgments(
                ResumeExpectation(**{**vars(expectation()), "run_token": "../bad"}))
        with self.assertRaisesRegex(ResumeAckError, "checkpoint_roles_invalid"):
            StrictResumeAcknowledgments(
                ResumeExpectation(**{**vars(expectation()), "checkpoint_sha256": {"gpt": GPT_SHA}}))


if __name__ == "__main__":
    unittest.main()
