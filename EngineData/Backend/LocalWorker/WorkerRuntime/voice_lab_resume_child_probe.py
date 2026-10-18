"""Opt-in, spawned-worker SoVITS checkpoint-load diagnostic.

NOT a Resume implementation. Normal My Voice training never imports or calls
this module. A developer must explicitly request probe mode with a validated,
locally trusted checkpoint plan. It does NOT alter GPT-SoVITS model or inference
weights. It substitutes only the trainer child's checkpoint loading boundary.

Crucial: pinned s2_train.py has a bare 'except' around load_checkpoint that
silently returns to pretrained weights. An ordinary exception cannot escape
that handler. In the explicitly armed child, a failed strict load therefore
terminates that child with a nonzero exit via os._exit; no fallback can run.

No output from this module alone is proof of an authentic worker-origin event
or a resumed training run. Parent source/process verification and GPU tests are
still mandatory, and the end-user Resume control remains disabled.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

from voice_lab_gpt_sovits import ENGINE_REVISION
from voice_lab_strict_resume_ack import ResumeExpectation

_EXIT_LOAD_FAILED = 78
_HEX64 = re.compile(r"[a-f0-9]{64}\Z")
_PLAN_LIMIT = 8192


class StrictChildLoadError(RuntimeError):
    pass


@dataclass(frozen=True)
class SoVITSChildPlan:
    run_token: str
    recovery_id: str
    dataset_id: str
    g_sha: str
    d_sha: str
    g_path: str
    d_path: str
    expected_epoch: int
    event_dir: str
    source_root: str

    @classmethod
    def from_json(cls, path: Path) -> "SoVITSChildPlan":
        if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= _PLAN_LIMIT:
            raise StrictChildLoadError("strict_resume_plan_invalid")
        raw = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(raw, dict) or set(raw) != set(cls.__dataclass_fields__):
            raise StrictChildLoadError("strict_resume_plan_schema_invalid")
        plan = cls(**raw)
        plan.validate()
        return plan

    def validate(self) -> None:
        for label in ("run_token", "recovery_id", "dataset_id", "g_sha", "d_sha"):
            if not isinstance(getattr(self, label), str) or not _HEX64.fullmatch(
                getattr(self, label)
            ):
                raise StrictChildLoadError(f"strict_resume_{label}_invalid")
        if type(self.expected_epoch) is not int or not 1 <= self.expected_epoch < 8:
            raise StrictChildLoadError("strict_resume_epoch_not_continuable")
        for label in ("g_path", "d_path"):
            path = Path(getattr(self, label))
            if not path.is_absolute() or path.is_symlink() or not path.is_file():
                raise StrictChildLoadError(f"strict_resume_{label}_untrusted")
        directory = Path(self.event_dir)
        if not directory.is_absolute() or directory.is_symlink() or not directory.is_dir():
            raise StrictChildLoadError("strict_resume_event_directory_untrusted")
        upstream = Path(self.source_root)
        if not upstream.is_absolute() or upstream.is_symlink() or not upstream.is_dir():
            raise StrictChildLoadError("strict_resume_source_root_untrusted")
        if Path(self.g_path).resolve() == Path(self.d_path).resolve():
            raise StrictChildLoadError("strict_resume_same_G_D_file")

    def expectation(self, gpt_sha: str, gpt_epoch: int, gpt_step: int) -> ResumeExpectation:
        expectation = ResumeExpectation(
            run_token=self.run_token,
            recovery_id=self.recovery_id,
            dataset_id=self.dataset_id,
            checkpoint_sha256={"sovits_G": self.g_sha, "sovits_D": self.d_sha, "gpt": gpt_sha},
            gpt_epoch=gpt_epoch,
            gpt_global_step=gpt_step,
            sovits_epoch=self.expected_epoch,
        )
        expectation.validate()
        return expectation


def hash_file(path: Path) -> str:
    if path.is_symlink() or not path.is_file() or not 0 < path.stat().st_size <= 8 * 1024**3:
        raise StrictChildLoadError("strict_resume_checkpoint_not_regular")
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(64 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


class StrictSoVITSChildMonitor:
    """Operates within the actual spawned SoVITS worker (one rank only).

    The optimizer callback fires AFTER the first real optimizer.step, not when
    the epoch starts; AMP may skip optimizer steps and must not claim progress.
    """

    def __init__(
        self,
        plan: SoVITSChildPlan,
        *,
        pid: int,
        decode: Callable[[Path], dict[str, Any]],
        emit: Callable[[dict[str, Any]], None],
        hard_abort: Callable[[int], None],
    ):
        plan.validate()
        if type(pid) is not int or pid <= 0:
            raise StrictChildLoadError("strict_resume_invalid_worker_pid")
        self.plan = plan
        self.pid = pid
        self.decode = decode
        self.emit = emit
        self.hard_abort = hard_abort
        self.loaded: dict[str, Any] = {}
        self.loaded_epoch: int | None = None
        self.current_epoch: int | None = None
        self.sent_start = False
        self.sequence = 0

    def _event(self, kind: str, role: str, **kwargs: Any) -> None:
        self.sequence += 1
        self.emit(
            {
                "schema_version": 1,
                "run_token": self.plan.run_token,
                "recovery_id": self.plan.recovery_id,
                "dataset_id": self.plan.dataset_id,
                "engine_revision": ENGINE_REVISION,
                "worker_pid": self.pid,
                "sequence": self.sequence,
                "kind": kind,
                "role": role,
                **kwargs,
            }
        )

    def _abort(self, reason: str) -> None:
        # In the real spawned process this is os._exit, NOT raise: upstream
        # catches BaseException and would otherwise load pretrained weights.
        try:
            self._event("fallback", "sovits", reason=reason)
        finally:
            self.hard_abort(_EXIT_LOAD_FAILED)
        raise StrictChildLoadError("strict_resume_hard_abort_returned")

    def load(
        self, role: str, path: str, model: Any, optimizer: Any, *, skip_optimizer: bool = False
    ) -> tuple[Any, Any, float, int]:
        try:
            if role not in ("sovits_D", "sovits_G") or role in self.loaded:
                raise StrictChildLoadError("checkpoint_role_out_of_order_or_duplicate")
            if role == "sovits_G" and "sovits_D" not in self.loaded:
                raise StrictChildLoadError("generator_before_discriminator")
            source = Path(path)
            expected = self.plan.d_path if role == "sovits_D" else self.plan.g_path
            expected_sha = self.plan.d_sha if role == "sovits_D" else self.plan.g_sha
            if (
                not source.is_file()
                or source.is_symlink()
                or not os.path.samefile(source, expected)
            ):
                raise StrictChildLoadError("checkpoint_path_mismatch")
            if hash_file(source) != expected_sha:
                raise StrictChildLoadError("checkpoint_hash_mismatch")
            if optimizer is None or skip_optimizer:
                raise StrictChildLoadError("optimizer_restore_skipped")
            payload = self.decode(source)  # trusted local torch.load(weights_only=True)
            if not isinstance(payload, dict):
                raise StrictChildLoadError("checkpoint_not_mapping")
            epoch = payload.get("iteration")
            if type(epoch) is not int or epoch != self.plan.expected_epoch:
                raise StrictChildLoadError("checkpoint_epoch_mismatch")
            model_state = payload.get("model")
            saved_optimizer = payload.get("optimizer")
            if not isinstance(model_state, dict) or not model_state:
                raise StrictChildLoadError("model_state_missing")
            if (
                not isinstance(saved_optimizer, dict)
                or not isinstance(saved_optimizer.get("state"), dict)
                or not saved_optimizer["state"]
                or not isinstance(saved_optimizer.get("param_groups"), list)
                or not saved_optimizer["param_groups"]
            ):
                raise StrictChildLoadError("optimizer_state_missing")
            target = model.module if hasattr(model, "module") else model
            actual_model = target.state_dict()
            if set(model_state) != set(actual_model) or any(
                not hasattr(model_state[k], "shape")
                or tuple(model_state[k].shape) != tuple(actual_model[k].shape)
                for k in actual_model
            ):
                raise StrictChildLoadError("strict_model_tensor_layout_mismatch")
            actual_groups = optimizer.state_dict().get("param_groups")
            if (
                not isinstance(actual_groups, list)
                or len(actual_groups) != len(saved_optimizer["param_groups"])
                or any(
                    not isinstance(saved, dict)
                    or not isinstance(saved.get("params"), list)
                    or not isinstance(active, dict)
                    or not isinstance(active.get("params"), list)
                    or len(saved["params"]) != len(active["params"])
                    for active, saved in zip(actual_groups, saved_optimizer["param_groups"])
                )
            ):
                raise StrictChildLoadError("optimizer_param_groups_mismatch")
            target.load_state_dict(model_state, strict=True)
            optimizer.load_state_dict(saved_optimizer)
            restored = optimizer.state_dict()
            if not isinstance(restored.get("state"), dict) or not restored["state"]:
                raise StrictChildLoadError("optimizer_state_not_loaded")
            # Re-hash after loading: files changed during deserialization cannot
            # be mistaken for a verified checkpoint.
            if hash_file(source) != expected_sha:
                raise StrictChildLoadError("checkpoint_changed_during_load")
            self.loaded[role] = optimizer
            self.loaded_epoch = epoch
            self._event(
                "checkpoint_loaded",
                role,
                checkpoint_sha256=expected_sha,
                checkpoint_epoch=epoch,
                model_loaded=True,
                optimizer_loaded=True,
            )
            return model, optimizer, float(payload["learning_rate"]), epoch
        except Exception as exc:
            self._abort(type(exc).__name__)
            raise AssertionError("unreachable") from exc

    def mark_epoch(self, epoch: int) -> None:
        if type(epoch) is not int or set(self.loaded) != {"sovits_G", "sovits_D"}:
            self._abort("training_epoch_or_checkpoint_pair_invalid")
        if not self.sent_start:
            if epoch != self.plan.expected_epoch + 1:
                self._abort("first_epoch_mismatch")
        elif self.current_epoch is None or epoch < self.current_epoch:
            self._abort("training_epoch_moved_backwards")
        self.current_epoch = epoch

    def after_first_generator_optimizer_step(self) -> None:
        if self.sent_start:
            return
        if self.current_epoch != self.plan.expected_epoch + 1 or set(self.loaded) != {
            "sovits_G",
            "sovits_D",
        }:
            self._abort("first_optimizer_step_wrong_epoch")
        self.sent_start = True
        self._event("training_started", "sovits", next_epoch=self.current_epoch)


def _child_record(path: Path, event: dict[str, Any]) -> None:
    """Per-worker append-only developer evidence. Not authenticated provenance."""
    line = json.dumps(event, separators=(",", ":"), sort_keys=True) + "\n"
    if len(line) > 2048:
        raise StrictChildLoadError("worker_ack_event_too_large")
    fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o600)
    try:
        with os.fdopen(fd, "a", encoding="utf-8") as stream:
            stream.write(line)
            stream.flush()
            os.fsync(stream.fileno())
    except BaseException:
        raise


def sovits_child_entry(rank: int, n_gpus: int, hps: Any, raw_plan: dict[str, Any]) -> None:
    """Top-level importable function for torch.multiprocessing.spawn on Windows."""
    plan = SoVITSChildPlan(**raw_plan)
    plan.validate()
    if n_gpus != 1 or rank != 0:
        os._exit(_EXIT_LOAD_FAILED)
    # Re-import the same pinned module only in the child. No parent-only hook
    # can silently claim that spawn's fresh interpreter loaded the checkpoint.
    import torch

    import s2_train
    import utils
    from voice_lab_resume_validation import _engine_source

    expected_script = (Path(plan.source_root) / "GPT_SoVITS" / "s2_train.py").resolve()
    if Path(s2_train.__file__).resolve() != expected_script:
        os._exit(_EXIT_LOAD_FAILED)
    _engine_source(Path(plan.source_root))

    def decode(path: Path) -> dict[str, Any]:
        return torch.load(path, map_location="cpu", weights_only=True)

    output = Path(plan.event_dir) / f"worker-{os.getpid()}-{plan.run_token[:12]}.jsonl"
    monitor = StrictSoVITSChildMonitor(
        plan,
        pid=os.getpid(),
        decode=decode,
        emit=lambda event: _child_record(output, event),
        hard_abort=os._exit,
    )
    original_load = utils.load_checkpoint
    original_train = s2_train.train_and_evaluate

    def strict_load(path: str, model: Any, optimizer: Any = None, skip_optimizer: bool = False):
        role = "sovits_D" if not monitor.loaded else "sovits_G"
        return monitor.load(role, path, model, optimizer, skip_optimizer=skip_optimizer)

    def training(*args: Any, **kwargs: Any) -> Any:
        # Pinned train_and_evaluate(rank, epoch, hps, nets, optims, ...).
        if len(args) < 5:
            monitor._abort("unexpected_upstream_train_signature")
        monitor.mark_epoch(args[1])
        generator_optimizer = args[4][0]
        original_step = generator_optimizer.step

        def first_step(*step_args: Any, **step_kwargs: Any):
            result = original_step(*step_args, **step_kwargs)
            monitor.after_first_generator_optimizer_step()
            return result

        generator_optimizer.step = first_step
        try:
            return original_train(*args, **kwargs)
        finally:
            generator_optimizer.step = original_step

    utils.load_checkpoint = strict_load
    s2_train.train_and_evaluate = training
    try:
        s2_train.run(rank, n_gpus, hps)
        if not monitor.sent_start:
            monitor._abort("no_actual_generator_optimizer_step")
    except BaseException:
        # Never let the upstream bare-except recovery convert a failed strict
        # run into an implicit pretrained run. This gate is developer-only.
        os._exit(_EXIT_LOAD_FAILED)
    finally:
        utils.load_checkpoint = original_load
        s2_train.train_and_evaluate = original_train


def install_opt_in_sovits_spawn_probe(
    source_root: Path,
    plan_file: Path,
) -> None:
    """Install only when explicitly invoked by the developer stage runner.

    Current normal My Voice path never calls this function. This function
    changes torch.multiprocessing.spawn *only in this one-shot stage process*.
    """
    from voice_lab_resume_validation import _engine_source

    _engine_source(source_root)  # exact pinned trainer files before patching
    plan = SoVITSChildPlan.from_json(plan_file)
    if Path(plan.source_root).resolve() != source_root.resolve():
        raise StrictChildLoadError("strict_resume_plan_engine_source_mismatch")
    import torch.multiprocessing as mp

    original_spawn = mp.spawn

    def strict_spawn(
        fn: Any,
        args: tuple = (),
        nprocs: int = 1,
        join: bool = True,
        daemon: bool = False,
        start_method: str = "spawn",
    ):
        if not (
            getattr(fn, "__name__", None) == "run"
            and nprocs == 1
            and join is True
            and isinstance(args, tuple)
            and len(args) == 2
            and args[0] == 1
        ):
            raise StrictChildLoadError("unexpected_spawn_configuration")
        return original_spawn(
            sovits_child_entry,
            args=(args[0], args[1], vars(plan)),
            nprocs=nprocs,
            join=join,
            daemon=daemon,
            start_method=start_method,
        )

    mp.spawn = strict_spawn
