from __future__ import annotations

import ast
from pathlib import Path

WORKER_ROOT = Path(__file__).resolve().parents[1]
MILMMT_PROVIDER_PATH = WORKER_ROOT / "milmmt_translation_provider.py"
IO_RUNTIME_PATH = WORKER_ROOT / "worker_io_runtime.py"


def _calls(path: Path, method: str) -> list[ast.Call]:
    tree = ast.parse(path.read_text(encoding="utf-8"))
    return [
        node
        for node in ast.walk(tree)
        if isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and node.func.attr == method
    ]


def _has_true_keyword(calls: list[ast.Call], keyword_name: str) -> bool:
    return any(
        keyword.arg == keyword_name
        and isinstance(keyword.value, ast.Constant)
        and keyword.value.value is True
        for call in calls
        for keyword in call.keywords
    )


def test_milmmt_generation_uses_cache_without_unused_result_wrapper() -> None:
    generate_calls = _calls(MILMMT_PROVIDER_PATH, "generate")
    assert generate_calls, "MiLMMT provider must own an explicit model.generate call"
    assert _has_true_keyword(generate_calls, "use_cache"), (
        "MiLMMT deterministic inference must explicitly enable KV cache"
    )
    assert not _has_true_keyword(generate_calls, "return_dict_in_generate"), (
        "MiLMMT translation only consumes generated token sequences and should not request an unused result wrapper"
    )


def test_asr_text_only_path_disables_timestamp_token_decoding() -> None:
    transcribe_calls = _calls(IO_RUNTIME_PATH, "transcribe")
    assert transcribe_calls, "worker IO runtime must own an explicit model.transcribe call"
    assert _has_true_keyword(transcribe_calls, "without_timestamps"), (
        "Meeting ASR consumes text only and must not spend decoding work on timestamp tokens"
    )


def test_milmmt_translation_reports_stage_level_inference_telemetry() -> None:
    source = MILMMT_PROVIDER_PATH.read_text(encoding="utf-8")
    for field in (
        '"tokenization_ms"',
        '"inference_ms"',
        '"decode_ms"',
        '"inference_tokens_per_second"',
    ):
        assert field in source, f"MiLMMT translation response must expose {field}"

    assert "tokenization_started = time.perf_counter()" in source
    assert "inference_started = time.perf_counter()" in source
    assert "decode_started = time.perf_counter()" in source
