from __future__ import annotations

import importlib.util
import io
import json
import subprocess
import sys
from pathlib import Path

import pytest

WORKER_PATH = Path(__file__).resolve().parents[1] / "realtime_local_worker.py"


def load_worker_module():
    spec = importlib.util.spec_from_file_location("translateit_protocol_worker", WORKER_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run_worker_line(raw: str) -> dict:
    completed = subprocess.run(
        [sys.executable, str(WORKER_PATH)],
        input=raw,
        text=True,
        capture_output=True,
        timeout=5,
        check=False,
    )
    assert completed.returncode == 0
    lines = [line for line in completed.stdout.splitlines() if line.strip()]
    assert len(lines) == 1
    return json.loads(lines[0])


@pytest.mark.parametrize(
    "raw",
    [
        '{"command":"ping","deadline_unix_ms":NaN}\n',
        '{"command":"ping","deadline_unix_ms":Infinity}\n',
        '{"command":"ping","deadline_unix_ms":-Infinity}\n',
        '{"command":"ping","deadline_unix_ms":1e309}\n',
    ],
)
def test_protocol_rejects_non_finite_json_numbers_before_dispatch(raw: str) -> None:
    payload = run_worker_line(raw)
    assert payload == {
        "ok": False,
        "stage": "worker_request",
        "blocker": "worker:non_finite_json_number",
    }


def test_protocol_normalizes_malformed_json_to_stable_request_blocker() -> None:
    payload = run_worker_line('{"command":"ping", bad-json}\n')
    assert payload == {
        "ok": False,
        "stage": "worker_request",
        "blocker": "worker:invalid_json",
    }


def test_protocol_bounds_unexpected_handler_diagnostics(monkeypatch) -> None:
    worker = load_worker_module()

    def fail_handler(_payload):
        raise RuntimeError((" provider failure " * 500) + "\x00tail")

    monkeypatch.setitem(worker.runtime.HANDLERS, "ping", fail_handler)
    stdin = io.StringIO('{"command":"ping"}\n')
    stdout = io.StringIO()
    monkeypatch.setattr(worker.sys, "stdin", stdin)
    monkeypatch.setattr(worker.sys, "stdout", stdout)

    assert worker.main() == 0
    payload = json.loads(stdout.getvalue().strip())
    assert payload["ok"] is False
    assert payload["stage"] == "worker_error"
    assert payload["blocker"] == "worker:handler_failed:RuntimeError"
    assert 0 < len(payload["note"]) <= worker.MAX_PROTOCOL_NOTE_CHARS
    assert "\x00" not in payload["note"]
    assert "\n" not in payload["note"]


def test_protocol_rejects_non_object_handler_response(monkeypatch) -> None:
    worker = load_worker_module()
    monkeypatch.setitem(worker.runtime.HANDLERS, "ping", lambda _payload: ["not", "an", "object"])
    stdin = io.StringIO('{"command":"ping"}\n')
    stdout = io.StringIO()
    monkeypatch.setattr(worker.sys, "stdin", stdin)
    monkeypatch.setattr(worker.sys, "stdout", stdout)

    assert worker.main() == 0
    payload = json.loads(stdout.getvalue().strip())
    assert payload == {
        "ok": False,
        "stage": "worker_error",
        "blocker": "worker:response_must_be_object",
    }


def test_protocol_rejects_oversized_line_before_handler_dispatch(monkeypatch) -> None:
    worker = load_worker_module()

    def must_not_run(_payload):
        raise AssertionError("oversized request reached handler dispatch")

    monkeypatch.setitem(worker.runtime.HANDLERS, "ping", must_not_run)
    oversized = '{"command":"ping","padding":"' + ("x" * worker.MAX_WORKER_REQUEST_BYTES) + '"}\n'
    stdin = io.StringIO(oversized)
    stdout = io.StringIO()
    monkeypatch.setattr(worker.sys, "stdin", stdin)
    monkeypatch.setattr(worker.sys, "stdout", stdout)

    assert worker.main() == 0
    payload = json.loads(stdout.getvalue().strip())
    assert payload["ok"] is False
    assert payload["stage"] == "worker_request"
    assert payload["blocker"] == "worker:request_too_large"
    assert payload["max_bytes"] == worker.MAX_WORKER_REQUEST_BYTES

def test_protocol_echoes_host_request_id_on_success_and_failure(monkeypatch) -> None:
    worker = load_worker_module()
    for outcome in (
        {"ok": True, "stage": "ping", "request_id": "handler-forged"},
        {"ok": False, "stage": "ping", "blocker": "worker:test"},
    ):
        monkeypatch.setitem(worker.runtime.HANDLERS, "ping", lambda _request: outcome)
        monkeypatch.setattr(
            worker.sys, "stdin",
            io.StringIO('{"command":"ping","request_id":"helper-42"}\n'),
        )
        captured = io.StringIO()
        monkeypatch.setattr(worker.sys, "stdout", captured)
        assert worker.main() == 0
        response = json.loads(captured.getvalue().strip())
        assert response["request_id"] == "helper-42"
        assert response["ok"] == outcome["ok"]


def test_protocol_rejects_invalid_request_id_before_dispatch(monkeypatch) -> None:
    worker = load_worker_module()
    monkeypatch.setitem(
        worker.runtime.HANDLERS, "ping",
        lambda _request: (_ for _ in ()).throw(AssertionError("must not dispatch")),
    )
    monkeypatch.setattr(
        worker.sys, "stdin",
        io.StringIO('{"command":"ping","request_id":"invalid/identity"}\n'),
    )
    captured = io.StringIO()
    monkeypatch.setattr(worker.sys, "stdout", captured)
    assert worker.main() == 0
    assert json.loads(captured.getvalue().strip()) == {
        "ok": False, "stage": "worker_request",
        "blocker": "worker:invalid_request_id",
    }


def test_protocol_keeps_missing_request_id_compatible_with_handshake() -> None:
    response = run_worker_line('{"command":"ping"}\n')
    assert response["ok"] is True
    assert response["stage"] == "ping"
    assert "request_id" not in response

