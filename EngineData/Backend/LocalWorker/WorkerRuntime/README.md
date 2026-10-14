# Backend Local Worker Runtime

This folder owns TranslateIT's single Python inference worker used by the Tauri/Rust application.

## Process

```text
realtime_local_worker.py
├─ realtime_local_worker_base.py
│  ├─ worker_runtime_common.py
│  └─ worker_io_runtime.py
└─ milmmt_translation_provider.py
```

`realtime_local_worker.py` is the only application-facing worker entrypoint. Supporting modules split responsibility without creating another process/runtime:

- `realtime_local_worker_base.py`: newline-JSON protocol, status composition, command dispatch and standalone Text orchestration;
- `worker_runtime_common.py`: paths, limits, language normalization, GPU/device probes and generic completion helpers;
- `worker_io_runtime.py`: Faster Whisper ASR and GPT-SoVITS selected-voice inference state/commands;
- `milmmt_translation_provider.py`: Indonesian ↔ English MiLMMT translation behavior.

The worker exposes:

```text
status
asr_preload
transcribe
translation_preload
translate
voice_actor_preflight
voice_actor_synthesize
```

Rust remains Meeting/session/audio and selected-voice profile authority. The Python worker does not own Windows virtual-audio routing or a second product architecture.

## Translation

```text
xiaomi-research/MiLMMT-46-1B-v1.0
revision 4fc480b6c58dec29c159dcdf9fde0f6d5c354995
one resident causal model
ID → EN and EN → ID
CUDA BF16 primary
CPU fallback = explicit degraded operation
```

Production rules:

- official Xiaomi translation prompt;
- deterministic `do_sample=false`;
- continuation-only causal decoding;
- source is never silently truncated;
- known incomplete/non-EOS generation fails closed;
- RuntimeAssets readiness requires `.translateit_model_revision` to match the pinned revision;
- no M2M100/Marian/second-translator fallback/router.

Meeting translation context is bounded by the Rust/session contract: outbound may include the last three committed own-voice ID→EN pairs; incoming remains context-free; standalone Text does not inherit Meeting context.

## ASR and Meeting voice

- ASR primary: Faster Whisper Large V3 Turbo.
- Optional ASR fallback: Faster Whisper Medium.
- Meeting voice inference: one GPT-SoVITS `voice_actor_*` contract.
- Built-in Male/Female: Rust prepares a selected actor profile from pinned LibriSpeech reference assets plus reviewed shared V2ProPlus pretrained weights; user training is not required.
- My Voice: optional trained GPT-SoVITS V2ProPlus actor promoted only after held-out evaluation and explicit user approval.
- Existing `voice_lab_*` Python filenames/error namespaces are legacy compatibility identifiers, not product vocabulary.

## Dependencies

```text
pyproject.toml  = dependency intent
uv.lock         = resolved dependency graph
.python-version = developer Python pin
```

Normal setup consumes the lock frozen. Do not edit dependency intent without regenerating/reviewing the same lock change.

Current dependency versions:

```text
Python       3.12.x
PyTorch      2.11.0 / 2.11.0+cu126 on Windows/Linux CUDA path
Transformers 4.57.6 exact pin
Tokenizers   0.22.2 resolved by uv.lock
Accelerate   1.14.0 resolved by uv.lock
```

Transformers 4.57.6 is the validated MiLMMT runtime version. Implementation/dependency details belong here and in their source/lock owners rather than `CONTEXT.md` or `next-action.md`.

## PyTorch allocator diagnostic

The existing `status` response includes `gpu.torch_allocator` as a read-only,
best-effort observation. It inspects an **already initialized** CUDA context
and never imports PyTorch, initializes CUDA or preloads a model just for metrics.

- `state=not_initialized`: no initialized PyTorch CUDA allocator to inspect;
  **not** evidence that the GPU is unused.
- `state=unavailable`: inspection is unsupported or failed; an explicit
  `reason` accompanies it. This cannot change readiness or activate CPU fallback.
- `state=measured`: current PyTorch device ordinal and allocator values in
  MiB (`allocated`, `reserved`, `peak_allocated`, `peak_reserved`).
  Peaks are process-lifetime values; they are not per-turn peaks.
- `device_memory.state=measured`: CUDA-reported instantaneous device
  `free_mib`, `total_mib` and derived `used_mib`. This is **not**
  PyTorch-specific consumption and may include CTranslate2/other processes.
  Device-memory failure retains valid allocator metrics, marking the nested
  observation unavailable with a typed, redacted `reason`.

Do not subtract allocator values from device usage and label the difference as
Faster Whisper memory: CUDA contexts, libraries, WDDM, other applications and
memory accounting can overlap or differ. On multi-GPU systems, the PyTorch
current-device ordinal alone does not establish CTranslate2 physical GPU
identity. No reading is a safe training reservation, a device-wide peak, or
proof of Meeting stability.

For the eventual coordinated TARGET_WINDOWS acceptance, pair the existing
per-stage Meeting timings (`asr_ms`, `translation_ms`, `tts_ms` and
scheduler waits) with read-only worker status snapshots after cold preload,
warm readiness, sustained Live and Stop, plus the existing independent
whole-device `nvidia-smi` / Windows GPU observations. Compare the same
pinned Local SHA and GPU identity. A single status snapshot cannot attribute
GPU usage to individual stages or capture transient peaks. Do not query
the full status path on every utterance or add a background sampler.

## Developer checks

Repository-only MiLMMT validation does not require model/GPU inference:

```powershell
python tools/translation_quality/validate_canonical_milmmt_repo.py
```

With the frozen developer environment available:

```powershell
uv sync --frozen
uv run --frozen ruff check .
uv run --frozen ruff format --check .
uv run --frozen pytest
```

Target runtime smoke, when explicitly required:

```powershell
.\run_realtime_worker_smoke.ps1 -ExpectedDevice Cuda
```

Static/CI success does not prove real CUDA latency/VRAM, linguistic quality, voice quality, microphone behavior, virtual-audio delivery or clean-machine installation.

## Model acquisition / packaging

`model_manifest.json` is the model inventory. `prepare_model_assets.py` validates full revisions, stages Hugging Face snapshots atomically and writes revision markers after successful replacement.

Release packaging explicitly includes the worker entrypoint/support modules; large Python/model/GPT-SoVITS/built-in reference assets remain controlled payload inputs outside Git.

## Runtime rules

- one persistent Python AI worker;
- one dependency project/lock;
- one Indonesian ↔ English translation model;
- one selected-Meeting-voice inference contract;
- no hidden CPU retry after unknown CUDA/model failure;
- model presence is not model correctness without exact revision evidence;
- output without verifiable completion is not successful output;
- repository/hosted checks and target-Windows checks remain separate.
