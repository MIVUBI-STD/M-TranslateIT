"""Revision-locked, fail-closed single-GPU training adapter for SAFE_8GB.

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
    elif relative == "GPT_SoVITS/s1_train.py":
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
    else:
        raise RuntimeError("my_voice:single_gpu_source_unsupported")
    return source
