"""Strict training-load acknowledgment *protocol*, not a resume launcher.

Developer-only, read-only validation of worker-origin acknowledgment evidence.
The pinned GPT-SoVITS SoVITS trainer uses torch.multiprocessing.spawn, and its
bare except can fall back to pretrained weights. A parent-only Python hook,
a successful process exit, or the mere presence of a checkpoint does NOT prove
that the child loaded optimizer state.

No production training invocation, upstream monkeypatch, model change,
checkpoint modification, or user-facing Resume action is enabled by this file.
A future opt-in trainer-child adapter must emit the events at the *actual*
load and first-step boundaries, and be observed under TARGET_WINDOWS testing.
The run token is a correlation identifier, NOT a cryptographic signature.
"""

from __future__ import annotations

from dataclasses import dataclass, field
import json
from pathlib import Path
import re
from typing import Any

from voice_lab_gpt_sovits import ENGINE_REVISION, GPT_EPOCHS, SOVITS_EPOCHS

_HEX = re.compile(r"[0-9a-f]{64}\Z")
_ROLES = ("sovits_D", "sovits_G", "gpt")
_MAX_EVENTS = 8
_MAX_JSONL_BYTES = 16 * 1024


class ResumeAckError(ValueError):
    """Invalid, contradictory, incomplete or unsupported worker evidence."""


def _identity(value: Any, name: str) -> str:
    if not isinstance(value, str) or not _HEX.fullmatch(value):
        raise ResumeAckError(f"{name}_invalid")
    return value


def _positive_int(value: Any, name: str, *, allow_zero: bool = False) -> int:
    if type(value) is not int or value < (0 if allow_zero else 1):
        raise ResumeAckError(f"{name}_invalid")
    return value


@dataclass(frozen=True)
class ResumeExpectation:
    """Created by an authorized run controller from VERIFIED, immutable inputs.

    The recovery archive, dataset, checkpoint hashes and expected epochs must
    already have passed the separate strict read-only archive audit.
    """
    run_token: str
    recovery_id: str
    dataset_id: str
    checkpoint_sha256: dict[str, str]
    gpt_epoch: int
    gpt_global_step: int
    sovits_epoch: int
    engine_revision: str = ENGINE_REVISION

    def validate(self) -> None:
        _identity(self.run_token, "run_token")
        _identity(self.recovery_id, "recovery_id")
        _identity(self.dataset_id, "dataset_id")
        if self.engine_revision != ENGINE_REVISION:
            raise ResumeAckError("engine_revision_mismatch")
        if type(self.checkpoint_sha256) is not dict or set(self.checkpoint_sha256) != set(_ROLES):
            raise ResumeAckError("checkpoint_roles_invalid")
        for role in _ROLES:
            _identity(self.checkpoint_sha256[role], f"{role}_sha256")
        if _positive_int(self.gpt_epoch, "gpt_epoch", allow_zero=True) >= GPT_EPOCHS:
            raise ResumeAckError("gpt_epoch_out_of_range")
        _positive_int(self.gpt_global_step, "gpt_global_step")
        if _positive_int(self.sovits_epoch, "sovits_epoch") >= SOVITS_EPOCHS:
            raise ResumeAckError("sovits_epoch_out_of_range")


@dataclass
class StrictResumeAcknowledgments:
    """Reject unknown/duplicate/mismatched worker events, no success on fallback.

    The *parent* can validate a complete event sequence, but cannot fabricate
    the missing worker events. A complete sequence is not sufficient to enable
    exact resume because SoVITS scheduler/AMP continuity remains unproven.
    """
    expected: ResumeExpectation
    _loaded: dict[str, dict[str, Any]] = field(default_factory=dict)
    _started: set[str] = field(default_factory=set)
    _failed: bool = False
    _last_sequence: int = 0

    def __post_init__(self) -> None:
        self.expected.validate()

    def observe(self, event: dict[str, Any]) -> None:
        if self._failed:
            raise ResumeAckError("acknowledgment_session_already_failed")
        try:
            self._validate_event(event)
        except ResumeAckError:
            self._failed = True
            raise

    def _validate_event(self, event: dict[str, Any]) -> None:
        if not isinstance(event, dict):
            raise ResumeAckError("event_not_object")
        required = {"schema_version", "run_token", "recovery_id", "dataset_id",
                    "engine_revision", "worker_pid", "sequence", "kind"}
        if not required <= event.keys():
            raise ResumeAckError("acknowledgment_fields_missing")
        if event["schema_version"] != 1:
            raise ResumeAckError("acknowledgment_schema_unsupported")
        for field, expectation in (
            ("run_token", self.expected.run_token),
            ("recovery_id", self.expected.recovery_id),
            ("dataset_id", self.expected.dataset_id),
            ("engine_revision", self.expected.engine_revision),
        ):
            if event[field] != expectation:
                raise ResumeAckError(f"acknowledgment_{field}_mismatch")
        pid = _positive_int(event["worker_pid"], "worker_pid")
        sequence = _positive_int(event["sequence"], "sequence")
        if sequence != self._last_sequence + 1 or sequence > _MAX_EVENTS:
            raise ResumeAckError("acknowledgment_sequence_invalid")
        self._last_sequence = sequence
        kind = event["kind"]
        if kind == "fallback":
            raise ResumeAckError("trainer_fell_back_to_pretrained_weights")
        if kind == "checkpoint_loaded":
            self._checkpoint_loaded(event, pid)
        elif kind == "training_started":
            self._training_started(event, pid)
        else:
            raise ResumeAckError("acknowledgment_kind_unsupported")

    def _checkpoint_loaded(self, event: dict[str, Any], pid: int) -> None:
        role = event.get("role")
        if role not in _ROLES or role in self._loaded:
            raise ResumeAckError("checkpoint_role_missing_or_duplicate")
        if event.get("checkpoint_sha256") != self.expected.checkpoint_sha256[role]:
            raise ResumeAckError("loaded_checkpoint_identity_mismatch")
        if (event.get("model_loaded") is not True
                or event.get("optimizer_loaded") is not True):
            raise ResumeAckError("model_or_optimizer_load_unconfirmed")
        epoch = _positive_int(event.get("checkpoint_epoch"), "checkpoint_epoch",
                              allow_zero=(role == "gpt"))
        if epoch != (self.expected.gpt_epoch if role == "gpt" else self.expected.sovits_epoch):
            raise ResumeAckError("checkpoint_epoch_mismatch")
        if role == "gpt":
            if event.get("scheduler_loaded") is not True:
                raise ResumeAckError("gpt_scheduler_load_unconfirmed")
            if event.get("global_step") != self.expected.gpt_global_step:
                raise ResumeAckError("gpt_checkpoint_global_step_mismatch")
        else:
            if self._started:
                raise ResumeAckError("checkpoint_loaded_after_training_started")
            other = "sovits_G" if role == "sovits_D" else "sovits_D"
            if other in self._loaded and pid != self._loaded[other]["worker_pid"]:
                raise ResumeAckError("sovits_pair_different_worker")
        self._loaded[role] = {"worker_pid": pid, "epoch": epoch}

    def _training_started(self, event: dict[str, Any], pid: int) -> None:
        role = event.get("role")
        if role == "sovits":
            if not {"sovits_G", "sovits_D"} <= self._loaded.keys():
                raise ResumeAckError("sovits_training_started_without_checkpoint_pair")
            if "sovits" in self._started:
                raise ResumeAckError("duplicate_sovits_training_start")
            if pid != self._loaded["sovits_G"]["worker_pid"]:
                raise ResumeAckError("sovits_training_worker_mismatch")
            if event.get("next_epoch") != self.expected.sovits_epoch + 1:
                raise ResumeAckError("sovits_epoch_continuation_invalid")
        elif role == "gpt":
            if "gpt" not in self._loaded or "gpt" in self._started:
                raise ResumeAckError("gpt_training_start_without_load")
            if pid != self._loaded["gpt"]["worker_pid"]:
                raise ResumeAckError("gpt_training_worker_mismatch")
            if (type(event.get("current_epoch")) is not int
                    or event["current_epoch"] < self.expected.gpt_epoch
                    or event.get("global_step") != self.expected.gpt_global_step):
                raise ResumeAckError("gpt_training_start_state_invalid")
        else:
            raise ResumeAckError("training_start_role_unknown")
        self._started.add(role)

    def summary(self) -> dict[str, Any]:
        sequence_complete = (not self._failed
                             and set(self._loaded) == set(_ROLES)
                             and self._started == {"gpt", "sovits"})
        blockers = []
        if self._failed:
            blockers.append("worker_resume_acknowledgment_rejected")
        if not sequence_complete:
            blockers.append("worker_training_load_evidence_incomplete")
        # Deliberate gate: no promoted training-run resume until actual native
        # validation, non-silent G/D loading and AMP/scheduler requirements.
        blockers.extend(("sovits_amp_scheduler_continuity_not_proven",
                         "native_resume_reproducibility_not_tested"))
        return {
            "schema_version": 1,
            "recovery_id": self.expected.recovery_id,
            "dataset_id": self.expected.dataset_id,
            "event_count": self._last_sequence,
            "acknowledgment_sequence_complete": sequence_complete,
            # JSONL evidence is not authenticated proof of the actual spawned
            # trainer loading checkpoints. Only a native producer can attest.
            "worker_provenance_verified": False,
            "trainer_load_verified": False,
            "resume_available": False,
            "blockers": ["worker_origin_not_verified", *blockers],
        }


def verify_worker_event_log(path: Path, expected: ResumeExpectation) -> dict[str, Any]:
    """Read-only bounded parser; not a substitute for an authenticated worker.

    No hidden fallback from a malformed/absent log to 'resume success'.
    """
    if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= _MAX_JSONL_BYTES:
        raise ResumeAckError("worker_event_log_missing_or_untrusted")
    session = StrictResumeAcknowledgments(expected)
    try:
        with path.open("r", encoding="utf-8") as stream:
            for line in stream:
                if len(line) > 2048:
                    raise ResumeAckError("worker_event_too_large")
                event = json.loads(line)
                session.observe(event)
    except ResumeAckError:
        raise
    except (ValueError, UnicodeError) as exc:
        raise ResumeAckError("worker_event_log_invalid") from exc
    return session.summary()
