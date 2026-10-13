"""Opt-in, single-GPU GPT Lightning checkpoint restoration probe.

Developer-only. Normal VoiceLab never enables this module. The pinned GPT
training source, TTS model, inference, saved actors, and user recordings are
untouched. Strictly validates one trusted full Lightning checkpoint and actual
Trainer-restored state. A missing/bad ckpt_path cannot fall back to training
from scratch in this developer-only path.

Important: a complete JSONL event trace is diagnostic, NOT authenticated
worker provenance and NOT permission to enable the user-facing Resume action.
Neither this probe nor GPT/SoVITS checkpoints prove AMP/scaler continuity.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path
import re
from typing import Any, Callable

from voice_lab_gpt_sovits import ENGINE_REVISION, GPT_EPOCHS
from voice_lab_resume_validation import _engine_source

_HEX = re.compile(r"[0-9a-f]{64}\Z")
_MAX_PLAN = 8192
_MAX_CHECKPOINT = 8 * 1024**3
_EXIT_STRICT_FAILURE = 78


class GptProbeError(RuntimeError):
    pass


def _digest(path: Path) -> str:
    if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= _MAX_CHECKPOINT:
        raise GptProbeError("gpt_checkpoint_not_trusted_regular_file")
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(64 * 1024), b""):
            h.update(block)
    return h.hexdigest()


@dataclass(frozen=True)
class GptWorkerPlan:
    run_token: str
    recovery_id: str
    dataset_id: str
    source_root: str
    checkpoint_path: str
    checkpoint_sha256: str
    config_path: str
    config_sha256: str
    expected_epoch: int
    expected_global_step: int
    event_dir: str

    @classmethod
    def read(cls, file: Path) -> "GptWorkerPlan":
        if file.is_symlink() or not file.is_file() or not 0 < file.stat().st_size <= _MAX_PLAN:
            raise GptProbeError("gpt_plan_untrusted")
        value = json.loads(file.read_text(encoding="utf-8"))
        if not isinstance(value, dict) or set(value) != set(cls.__dataclass_fields__):
            raise GptProbeError("gpt_plan_schema_invalid")
        plan = cls(**value)
        plan.validate()
        return plan

    def validate(self) -> None:
        for name in ("run_token", "recovery_id", "dataset_id", "checkpoint_sha256", "config_sha256"):
            item = getattr(self, name)
            if not isinstance(item, str) or not _HEX.fullmatch(item):
                raise GptProbeError(f"gpt_{name}_invalid")
        if (type(self.expected_epoch) is not int
                or not 0 <= self.expected_epoch < GPT_EPOCHS
                or type(self.expected_global_step) is not int
                or self.expected_global_step < 1):
            raise GptProbeError("gpt_saved_progress_invalid")
        for name in ("source_root", "checkpoint_path", "config_path", "event_dir"):
            path = Path(getattr(self, name))
            if not path.is_absolute() or path.is_symlink():
                raise GptProbeError(f"gpt_{name}_not_absolute_or_symlink")
            if name in ("source_root", "event_dir"):
                if not path.is_dir():
                    raise GptProbeError(f"gpt_{name}_missing")
            elif not path.is_file():
                raise GptProbeError(f"gpt_{name}_missing")
        if Path(self.checkpoint_path).resolve() == Path(self.config_path).resolve():
            raise GptProbeError("gpt_checkpoint_equals_config")


def _optimizer_ok(data: Any) -> bool:
    return (isinstance(data, dict)
            and isinstance(data.get("state"), dict) and bool(data["state"])
            and isinstance(data.get("param_groups"), list)
            and bool(data["param_groups"]))


def _same_state(saved: Any, current: Any, *, depth: int = 0,
                budget: list[int] | None = None) -> bool:
    """Exact, bounded developer-only comparison; any unsupported type fails."""
    if budget is None:
        budget = [200_000]
    budget[0] -= 1
    if budget[0] < 0 or depth > 24:
        return False
    if hasattr(saved, "detach") or hasattr(current, "detach"):
        try:
            import torch
            if not (isinstance(saved, torch.Tensor) and isinstance(current, torch.Tensor)):
                return False
            if tuple(saved.shape) != tuple(current.shape):
                return False
            return bool(torch.equal(saved.detach().cpu(), current.detach().cpu()))
        except (ImportError, RuntimeError, TypeError):
            return False
    if isinstance(saved, dict) and isinstance(current, dict):
        return (saved.keys() == current.keys()
                and all(_same_state(saved[k], current[k], depth=depth+1, budget=budget)
                        for k in saved))
    if isinstance(saved, (list, tuple)) and isinstance(current, (list, tuple)):
        return (len(saved) == len(current)
                and all(_same_state(a, b, depth=depth+1, budget=budget)
                        for a, b in zip(saved, current)))
    if type(saved) in (str, int, bool, float, type(None)) and type(saved) is type(current):
        return saved == current
    return False


class GptWorkerWitness:
    """Receives actual Trainer state at on_train_start and on_train_batch_end.

    A successful optimizer.step() by itself is insufficient: Lightning must
    also advance global_step, so AMP-skipped steps cannot count.
    """

    def __init__(
        self, plan: GptWorkerPlan, *, pid: int,
        emit: Callable[[dict[str, Any]], None],
        abort: Callable[[int], None],
        decode: Callable[[Path], dict[str, Any]],
    ):
        plan.validate()
        if type(pid) is not int or pid <= 0:
            raise GptProbeError("gpt_worker_pid_invalid")
        self.plan = plan
        self.pid = pid
        self.emit = emit
        self.abort = abort
        self.decode = decode
        self.loaded = False
        self.optimizer_stepped = False
        self.started = False
        self.sequence = 0

    def _emit(self, kind: str, **extra: Any) -> None:
        self.sequence += 1
        self.emit({
            "schema_version": 1, "run_token": self.plan.run_token,
            "recovery_id": self.plan.recovery_id, "dataset_id": self.plan.dataset_id,
            "engine_revision": ENGINE_REVISION, "worker_pid": self.pid,
            "sequence": self.sequence, "kind": kind, "role": "gpt", **extra,
        })

    def _fail(self, reason: str) -> None:
        try:
            self._emit("fallback", reason=reason)
        finally:
            self.abort(_EXIT_STRICT_FAILURE)
        raise GptProbeError("gpt_hard_abort_returned")

    def preflight(self, ckpt_path: Any) -> dict[str, Any]:
        try:
            if not isinstance(ckpt_path, (str, Path)):
                raise GptProbeError("gpt_explicit_checkpoint_required")
            selected = Path(ckpt_path)
            if (selected.is_symlink()
                    or not os.path.samefile(selected, self.plan.checkpoint_path)
                    or _digest(selected) != self.plan.checkpoint_sha256
                    or _digest(Path(self.plan.config_path)) != self.plan.config_sha256):
                raise GptProbeError("gpt_checkpoint_or_config_identity_mismatch")
            saved = self.decode(selected)
            if not isinstance(saved, dict):
                raise GptProbeError("gpt_checkpoint_payload_invalid")
            if (saved.get("epoch") != self.plan.expected_epoch
                    or saved.get("global_step") != self.plan.expected_global_step
                    or not isinstance(saved.get("state_dict"), dict)
                    or not saved["state_dict"]
                    or not isinstance(saved.get("optimizer_states"), list)
                    or not saved["optimizer_states"]
                    or not all(_optimizer_ok(x) for x in saved["optimizer_states"])
                    or not isinstance(saved.get("lr_schedulers"), list)
                    or not saved["lr_schedulers"]):
                raise GptProbeError("gpt_checkpoint_training_state_invalid")
            if _digest(selected) != self.plan.checkpoint_sha256:
                raise GptProbeError("gpt_checkpoint_changed_during_preflight")
            return saved
        except Exception as exc:
            self._fail(type(exc).__name__)
            raise AssertionError("gpt_abort_unreachable") from exc

    def restored(self, trainer: Any, model: Any, saved: dict[str, Any]) -> None:
        try:
            if self.loaded:
                raise GptProbeError("gpt_duplicate_restore_acknowledgment")
            if (getattr(trainer, "global_step", None) != self.plan.expected_global_step
                    or type(getattr(trainer, "current_epoch", None)) is not int
                    or not self.plan.expected_epoch <= trainer.current_epoch <= self.plan.expected_epoch + 1):
                raise GptProbeError("gpt_trainer_progress_not_restored")
            if not _same_state(saved["state_dict"], model.state_dict()):
                raise GptProbeError("gpt_model_state_not_restored")
            optimizers = list(getattr(trainer, "optimizers", []))
            if len(optimizers) != len(saved["optimizer_states"]):
                raise GptProbeError("gpt_optimizer_count_mismatch")
            for optimizer, state in zip(optimizers, saved["optimizer_states"]):
                if not _same_state(state, optimizer.state_dict()):
                    raise GptProbeError("gpt_optimizer_state_not_restored")
            configs = list(getattr(trainer, "lr_scheduler_configs", []))
            if len(configs) != len(saved["lr_schedulers"]):
                raise GptProbeError("gpt_scheduler_count_mismatch")
            for cfg, state in zip(configs, saved["lr_schedulers"]):
                scheduler = getattr(cfg, "scheduler", None)
                if scheduler is None or not _same_state(state, scheduler.state_dict()):
                    raise GptProbeError("gpt_scheduler_state_not_restored")
            self.loaded = True
            self._emit(
                "checkpoint_loaded",
                checkpoint_sha256=self.plan.checkpoint_sha256,
                checkpoint_epoch=self.plan.expected_epoch,
                global_step=self.plan.expected_global_step,
                model_loaded=True, optimizer_loaded=True, scheduler_loaded=True,
            )
        except Exception as exc:
            self._fail(type(exc).__name__)
            raise AssertionError("gpt_abort_unreachable") from exc

    def after_optimizer_step(self) -> None:
        if not self.loaded:
            self._fail("gpt_optimizer_step_before_checkpoint_restored")
        self.optimizer_stepped = True

    def after_batch(self, trainer: Any) -> None:
        if self.started or not self.optimizer_stepped:
            return
        step = getattr(trainer, "global_step", None)
        epoch = getattr(trainer, "current_epoch", None)
        if (type(step) is not int or step <= self.plan.expected_global_step
                or type(epoch) is not int or epoch < self.plan.expected_epoch):
            self._fail("gpt_first_optimizer_step_did_not_advance_global_step")
        self.started = True
        self._emit(
            "training_started", current_epoch=epoch,
            # Expected *restored* global_step, as required by existing protocol.
            global_step=self.plan.expected_global_step,
            observed_global_step=step,
        )

    def finished(self) -> None:
        if not self.started:
            self._fail("gpt_checkpoint_training_never_advanced")


def _record(path: Path, event: dict[str, Any]) -> None:
    """Append-only diagnostics; a text file is NOT authenticated attestation."""
    line = json.dumps(event, separators=(",", ":"), sort_keys=True) + "\n"
    if len(line) > 2048:
        raise GptProbeError("gpt_ack_event_too_large")
    fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o600)
    with os.fdopen(fd, "a", encoding="utf-8") as out:
        out.write(line)
        out.flush()
        os.fsync(out.fileno())


def install_opt_in_gpt_lightning_probe(source_root: Path, plan_file: Path) -> None:
    """Patch only the explicit developer stage process, never production."""
    _engine_source(source_root)
    plan = GptWorkerPlan.read(plan_file)
    if Path(plan.source_root).resolve() != source_root.resolve():
        raise GptProbeError("gpt_plan_engine_root_mismatch")
    if _digest(Path(plan.checkpoint_path)) != plan.checkpoint_sha256:
        raise GptProbeError("gpt_checkpoint_checksum_invalid")
    if _digest(Path(plan.config_path)) != plan.config_sha256:
        raise GptProbeError("gpt_config_checksum_invalid")
    import torch
    import pytorch_lightning as pl
    from pytorch_lightning.callbacks import Callback

    # One GPU only: do not create unobserved ranks or DDP child processes.
    if not torch.cuda.is_available() or torch.cuda.device_count() != 1:
        raise GptProbeError("gpt_probe_requires_one_cuda_device")

    original_trainer = pl.Trainer
    filename = Path(plan.event_dir) / f"gpt-worker-{os.getpid()}-{plan.run_token[:12]}.jsonl"
    witness = GptWorkerWitness(
        plan, pid=os.getpid(),
        emit=lambda event: _record(filename, event),
        abort=os._exit,
        decode=lambda path: torch.load(path, map_location="cpu", weights_only=True),
    )

    class StrictCallback(Callback):
        def __init__(self):
            super().__init__()
            self.original_step: list[tuple[Any, Any]] = []
            self.saved: dict[str, Any] | None = None

        def set_checkpoint(self, path: Any) -> None:
            self.saved = witness.preflight(path)

        def on_train_start(self, trainer: Any, pl_module: Any) -> None:
            if self.saved is None:
                witness._fail("gpt_restore_preflight_missing")
            witness.restored(trainer, pl_module, self.saved)
            for optimizer in trainer.optimizers:
                original_step = optimizer.step

                def stepped(*args: Any, __original=original_step, **kwargs: Any):
                    result = __original(*args, **kwargs)
                    witness.after_optimizer_step()
                    return result

                optimizer.step = stepped
                self.original_step.append((optimizer, original_step))

        def on_train_batch_end(self, trainer: Any, pl_module: Any,
                               outputs: Any, batch: Any, batch_idx: int) -> None:
            witness.after_batch(trainer)

        def on_train_end(self, trainer: Any, pl_module: Any) -> None:
            witness.finished()

        def teardown(self, trainer: Any, pl_module: Any, stage: str) -> None:
            for optimizer, original in self.original_step:
                optimizer.step = original
            self.original_step.clear()

    hook = StrictCallback()

    class StrictTrainer(original_trainer):
        def __init__(self, *args: Any, **kwargs: Any):
            callbacks = list(kwargs.pop("callbacks", []) or [])
            callbacks.append(hook)
            super().__init__(*args, callbacks=callbacks, **kwargs)

        def fit(self, model: Any, *args: Any, ckpt_path: Any = None,
                **kwargs: Any) -> Any:
            hook.set_checkpoint(ckpt_path)
            try:
                result = super().fit(model, *args, ckpt_path=ckpt_path, **kwargs)
                witness.finished()
                return result
            except BaseException:
                os._exit(_EXIT_STRICT_FAILURE)

    pl.Trainer = StrictTrainer
