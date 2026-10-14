"""Revision-locked, fail-closed single-GPU training adapter for SAFE_CUDA.

The original GPT-SoVITS V2ProPlus sources stay immutable. This adapter can
only transform the exact reviewed Git blob revisions; drift is a hard error.
No claim of Windows training success, performance or memory safety is made.
"""
from __future__ import annotations

import hashlib
from pathlib import Path

SOURCE_BLOBS = {
    "GPT_SoVITS/s2_train.py": "333e6a05aedd9f62009ecd6012518116d1326a93",
    "GPT_SoVITS/s1_train.py": "1176f0bcef869d2f66574d47d9a55d0cc7e1ac0c",
}


def _replace_one(source: str, expected: str, replacement: str, label: str) -> str:
    if source.count(expected) != 1:
        raise RuntimeError(f"my_voice:single_gpu_source_drift:{label}")
    return source.replace(expected, replacement, 1)


def _verified_upstream_source(path: Path, relative: str) -> str:
    expected_blob = SOURCE_BLOBS.get(relative)
    if expected_blob is None:
        raise RuntimeError("my_voice:single_gpu_source_unsupported")
    raw = path.read_bytes()
    # Git blob identity, not a plain-file SHA-1: includes blob type and size.
    header = b"blob " + str(len(raw)).encode("ascii") + bytes([0])
    actual_blob = hashlib.sha1(header + raw).hexdigest()
    if actual_blob != expected_blob:
        raise RuntimeError(f"my_voice:single_gpu_source_revision_mismatch:{relative}")
    return raw.decode("utf-8")


def single_gpu_source(path: Path, relative: str) -> str:
    source = _verified_upstream_source(path, relative)
    if relative == "GPT_SoVITS/s2_train.py":
        source = _replace_one(
            source,
            """import utils

hps = utils.get_hparams(stage=2)""",
            """import utils
from my_voice_checkpoint_atomic import install_sovits_atomic_writer
install_sovits_atomic_writer(utils)

hps = utils.get_hparams(stage=2)""",
            "sovits_atomic_optimizer_writer",
        )
        source = _replace_one(
            source,
            "from process_ckpt import savee",
            """from process_ckpt import savee
import process_ckpt
from my_voice_checkpoint_atomic import install_sovits_inference_writer
install_sovits_inference_writer(process_ckpt)""",
            "sovits_atomic_inference_weights",
        )
        source = _replace_one(
            source,
            'os.environ["CUDA_VISIBLE_DEVICES"] = hps.train.gpu_numbers.replace("-", ",")',
            'os.environ["CUDA_VISIBLE_DEVICES"] = os.environ["TRANSLATEIT_MYVOICE_GPU_UUID"]',
            "sovits_preserve_physical_gpu_uuid",
        )
        source = _replace_one(
            source,
            """    mp.spawn(
        run,
        nprocs=n_gpus,
        args=(
            n_gpus,
            hps,
        ),
    )""",
            """    if n_gpus != 1 or not torch.cuda.is_available():
        raise RuntimeError("my_voice:requires_exactly_one_cuda_gpu")
    # In-process, single-GPU training. No CUDA DDP/Gloo child workers.
    run(0, 1, hps)""",
            "sovits_spawn",
        )
        source = _replace_one(
            source,
            """    dist.init_process_group(
        backend="gloo" if os.name == "nt" or not torch.cuda.is_available() else "nccl",
        init_method="env://?use_libuv=False",
        world_size=n_gpus,
        rank=rank,
    )""",
            """    if n_gpus != 1:
        raise RuntimeError("my_voice:single_gpu_only")""",
            "sovits_ddp_init",
        )
        source = _replace_one(
            source,
            """    if torch.cuda.is_available():
        net_g = DDP(net_g, device_ids=[rank], find_unused_parameters=True)
        net_d = DDP(net_d, device_ids=[rank], find_unused_parameters=True)
    else:
        net_g = net_g.to(device)
        net_d = net_d.to(device)""",
            """    if not torch.cuda.is_available():
        raise RuntimeError("my_voice:cuda_disappeared_during_training")
    # net_g and net_d are already moved to this CUDA device above.""",
            "sovits_ddp_wrap",
        )
        source = _replace_one(
            source, "net_g.module.load_state_dict(", "net_g.load_state_dict(",
            "sovits_pretrained_g",
        )
        source = _replace_one(
            source, "net_d.module.load_state_dict(", "net_d.load_state_dict(",
            "sovits_pretrained_d",
        )
        source = _replace_one(
            source, "generator.module.infer(", "generator.infer(",
            "sovits_optional_eval",
        )
        # Full G/D optimizer checkpoints use distinct step filenames and a
        # paired SHA-256 receipt published *after* both saves complete.
        source = _replace_one(
            source,
            '''    if rank == 0:
        logger.info("====> Epoch: {}".format(epoch))''',
            '''    if rank == 0 and epoch % hps.train.save_every_epoch == 0:
        from my_voice_checkpoint_pair import publish_pair
        from pathlib import Path
        publish_pair(
            Path(hps.data.exp_dir) / ("logs_s2_" + hps.model.version),
            global_step, epoch,
        )
    if rank == 0:
        logger.info("====> Epoch: {}".format(epoch))''',
            "sovits_paired_optimizer_receipt",
        )
        # Only yield AFTER the configured SoVITS epoch checkpoint save. It
        # remains an UNVERIFIED recovery checkpoint, never an optimizer resume.
        source = _replace_one(
            source,
            '''        scheduler_g.step()
        scheduler_d.step()
    print("training done")''',
            '''        scheduler_g.step()
        scheduler_d.step()
        from my_voice_epoch_progress import publish_epoch_activity
        publish_epoch_activity("sovits", epoch)
        from my_voice_cooperative import sovits_epoch_boundary
        if sovits_epoch_boundary(epoch, hps.train.save_every_epoch,
                                 hps.data.exp_dir, hps.model.version):
            print("my_voice:cooperative_checkpoint_boundary:sovits", flush=True)
            return
    print("training done")''',
            "sovits_cooperative_epoch_boundary",
        )
    elif relative == "GPT_SoVITS/s1_train.py":
        source = _replace_one(
            source,
            'from process_ckpt import my_save',
            'from my_voice_checkpoint_atomic import atomic_gpt_inference_save as my_save',
            "gpt_atomic_inference_weights",
        )
        source = _replace_one(
            source,
            '        devices=-1 if torch.cuda.is_available() else 1,',
            '        devices=1,',
            "gpt_device_count",
        )
        source = _replace_one(
            source,
            '''        strategy=DDPStrategy(process_group_backend="nccl" if platform.system() != "Windows" else "gloo")
        if torch.cuda.is_available()
        else "auto",''',
            '        strategy="auto",',
            "gpt_ddp_strategy",
        )
        # Use the pinned MyVoice ModelCheckpoint callback itself rather than
        # another callback whose ordering against ModelCheckpoint may change
        # across Lightning versions. The safe-stop check executes only AFTER
        # the current checkpoint writer returned successfully.
        source = _replace_one(
            source,
            "            self._save_last_checkpoint(trainer, monitor_candidates)",
            """            self._save_last_checkpoint(trainer, monitor_candidates)
            from my_voice_cooperative import gpt_epoch_boundary
            gpt_epoch_boundary(
                trainer, self.config["train"]["save_every_n_epoch"], str(self.dirpath)
            )
            from my_voice_epoch_progress import publish_epoch_activity
            publish_epoch_activity("gpt", trainer.current_epoch + 1)""",
            "gpt_cooperative_after_checkpoint",
        )
        source = _replace_one(
            source,
            '''    trainer: Trainer = Trainer(''',
            '''    from my_voice_checkpoint_io import create_atomic_lightning_checkpoint_io

    trainer: Trainer = Trainer(''',
            "gpt_local_checkpoint_io_import",
        )
        source = _replace_one(
            source,
            '        monitor="top_3_acc",',
            '        monitor=None,',
            "gpt_full_checkpoint_not_metric_dependent",
        )
        source = _replace_one(
            source,
            '''        save_on_train_epoch_end=True,''',
            '''        save_on_train_epoch_end=True,
        save_last=False,''',
            "gpt_no_redundant_last_checkpoint",
        )
        source = _replace_one(
            source,
            "        callbacks=[ckpt_callback],",
            """        callbacks=[ckpt_callback],
        plugins=[create_atomic_lightning_checkpoint_io()],""",
            "gpt_full_optimizer_atomic_plugin",
        )
    else:
        raise RuntimeError("my_voice:single_gpu_source_unsupported")
    return source
