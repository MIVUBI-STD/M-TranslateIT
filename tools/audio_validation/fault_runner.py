"""Explicit, no-model fault contract runner using existing audio validation owners.

This executes only deterministic protocol, WAV, and proof-gating cases.
It does not launch the application, change production behavior, invoke CI,
simulate successful model inference, or claim Windows/native device proof.
"""
from __future__ import annotations

import argparse
import json
import queue
import subprocess
import sys
import tempfile
import wave
from pathlib import Path

from harness import decode_wave, write_wave, translation_receipt_complete, voice_receipt_valid
from worker_runner import WorkerTransportError, await_worker_frame, validate_response_frame

MATRIX = Path(__file__).with_name("fault_matrix.json")
RECEIPT_SCHEMA = "translateit.audio_fault_results.v1"


def matrix_cases(path: Path = MATRIX) -> list[dict]:
    source = json.loads(path.read_text(encoding="utf-8"))
    rows = source.get("cases") if isinstance(source, dict) else None
    if not isinstance(source, dict) or source.get("schema") != "translateit.audio_fault_plan.v1" or not isinstance(rows, list):
        raise ValueError("invalid canonical audio fault matrix")
    seen = set()
    for row in rows:
        if not isinstance(row, dict) or row.get("id") in seen:
            raise ValueError("duplicate/malformed fault case")
        name = row.get("id", "")
        if not isinstance(name, str) or not name.startswith("F") or not name[1:].isdigit():
            raise ValueError("invalid fault case identifier")
        seen.add(name)
    if not rows:
        raise ValueError("empty audio fault matrix")
    return rows


def frame(payload: dict) -> bytes:
    return (json.dumps(payload, separators=(",", ":")) + "\n").encode()


def assert_rejected(payload: bytes | None, *, request_id: str = "fixture-1",
                    command: str = "transcribe") -> None:
    try:
        validate_response_frame(payload, request_id=request_id, command=command)
    except WorkerTransportError:
        return
    raise AssertionError("invalid worker protocol response was accepted")


def test_worker_malformed_response() -> dict:
    valid = {"ok": True, "stage": "transcribe", "request_id": "fixture-1"}
    assert_rejected(frame({**valid, "request_id": "fixture-old"}))
    assert_rejected(frame({**valid, "stage": "translate"}))
    assert_rejected(frame({**valid, "ok": "true"}))
    assert_rejected(b"{broken-json}\n")
    assert_rejected(frame(valid)[:-1])
    assert_rejected(None)
    assert_rejected(b"x" * 1_000_001 + b"\n")
    assert validate_response_frame(frame(valid), request_id="fixture-1",
                                   command="transcribe")["ok"] is True
    assert_rejected(frame({"ok": True, "stage": "transcribe",
                           "request_id": "fixture-1"}), command="status")
    assert validate_response_frame(
        frame({"ok": False, "stage": "local_realtime_worker_preflight",
               "request_id": "fixture-1"}), request_id="fixture-1",
        command="status")["ok"] is False
    return {"invalid_responses_rejected": 8, "valid_responses_admitted": 2}


def test_asr_empty_response() -> dict:
    # Worker protocol may correctly carry an ASR stage with ok=False. A blank
    # "successful" transcript must never be graded into subsequent stages.
    from harness import word_error_rate
    raw = frame({"ok": False, "stage": "transcribe", "request_id": "fixture-1",
                 "blocker": "asr:empty_transcript", "transcript_text": ""})
    reply = validate_response_frame(raw, request_id="fixture-1", command="transcribe")
    assert reply["ok"] is False and not reply["transcript_text"]
    assert word_error_rate("Saya tidak setuju.", "") > 0
    return {"blocked_stage": "asr:empty_transcript"}


def test_wav_format_rejection() -> dict:
    with tempfile.TemporaryDirectory(prefix="translateit-fault-") as td:
        target = Path(td) / "public.wav"
        write_wave(target, [100, -100] * 150)
        values, rate = decode_wave(target)
        assert len(values) == 300 and rate == 16000
        with wave.open(str(target), "wb") as writer:
            writer.setnchannels(2)
            writer.setsampwidth(2)
            writer.setframerate(16000)
            writer.writeframes(b"\0" * 600)
        try:
            decode_wave(target)
        except ValueError:
            pass
        else:
            raise AssertionError("stereo WAV incorrectly accepted as canonical mono")
        target.write_bytes(b"RIFFbad")
        try:
            decode_wave(target)
        except (ValueError, EOFError, wave.Error):
            pass
        else:
            raise AssertionError("truncated WAV incorrectly accepted")
    return {"rejected": ["wrong_channel_count", "truncated_riff"]}



def test_worker_timeout() -> dict:
    # Empty bounded queue models transport non-response; does not start AI.
    inbox: queue.Queue[bytes | None] = queue.Queue()
    try:
        await_worker_frame(inbox, 0.001)
    except WorkerTransportError as exc:
        if "deadline_exceeded" not in str(exc):
            raise AssertionError("timeout was not classified correctly") from exc
    else:
        raise AssertionError("missing response was not rejected")
    return {"failure": "worker:response_deadline_exceeded",
            "boundary": "SIMULATED_QUEUE_ONLY_NO_NATIVE_COLD_START"}


def test_worker_exit() -> dict:
    inbox: queue.Queue[bytes | None] = queue.Queue()
    inbox.put_nowait(None)
    try:
        await_worker_frame(inbox, 0.01)
    except WorkerTransportError as exc:
        if "stdout_closed" not in str(exc):
            raise AssertionError("EOF was not classified correctly") from exc
    else:
        raise AssertionError("closed worker stdout was not rejected")
    return {"failure": "worker:stdout_closed",
            "boundary": "SIMULATED_QUEUE_ONLY_NO_NATIVE_PROCESS_EXIT"}



def test_translation_missing_eos() -> dict:
    correct = {"ok": True, "direction_pair": "id->en",
               "complete": True, "finished_with_eos": True,
               "translated_text": "Please do not delete the data."}
    if not translation_receipt_complete(correct, "id->en"):
        raise AssertionError("valid translation contract was rejected")
    for broken in (
        {**correct, "complete": False},
        {**correct, "finished_with_eos": False},
        {**correct, "direction_pair": "en->id"},
        {**correct, "translated_text": "  "},
    ):
        if translation_receipt_complete(broken, "id->en"):
            raise AssertionError("partial/wrong-direction translation accepted")
    return {"rejected": ["missing_complete", "missing_eos",
                          "wrong_direction", "blank_translation"],
            "boundary": "SOURCE_RECEIPT_ONLY_NO_REAL_TRANSLATION_INFERENCE"}


def test_tts_silent_or_wrong_actor() -> dict:
    good = {"ok": True, "wav_sha256": "a" * 64,
            "duration_ms": 1500, "rms": 0.09, "peak": 0.65,
            "rate_hz": 32000}
    preflight = {"ok": True}
    if not voice_receipt_valid(good, preflight):
        raise AssertionError("valid synthetic TTS receipt contract rejected")
    for broken, gate in (
        ({**good, "rms": 0.0}, preflight),
        ({**good, "wav_sha256": ""}, preflight),
        ({**good, "duration_ms": 0}, preflight),
        (good, {"ok": False}),
        (good, None),
    ):
        if voice_receipt_valid(broken, gate):
            raise AssertionError("unsupported TTS receipt accepted")
    return {"rejected": ["silent_wave", "missing_hash", "zero_duration",
                          "preflight_failure", "preflight_missing"],
            "boundary": "SOURCE_RECEIPT_ONLY_NO_REAL_ACTOR_OR_WAV"}



def test_real_child_process_exit() -> dict:
    """Launch an isolated Python child, then observe a real process EOF.

    This checks bounded process and pipe behavior only. It deliberately does
    not kill or replace the pinned AI worker, nor claim native recovery.
    """
    script = "import os, sys; sys.stdout.flush(); os._exit(17)"
    child = subprocess.Popen([sys.executable, "-I", "-u", "-c", script],
                             stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    try:
        frame = child.stdout.readline(1024) if child.stdout else b""
        exit_code = child.wait(timeout=5)
        if exit_code != 17 or frame:
            raise AssertionError("isolated child did not exit before a response")
        assert_rejected(frame, command="status")
        return {"exit_code": exit_code, "response": "EOF_REJECTED",
                "boundary": "REAL_ISOLATED_PYTHON_CHILD_NOT_CANONICAL_AI_WORKER"}
    finally:
        if child.poll() is None:
            child.kill()
            child.wait(timeout=5)
        if child.stdout:
            child.stdout.close()


# Only implemented, actually executed fault checks may return PASS. Remaining
# cases stay NOT_EXECUTED by construction until their owning injector exists.
RUNNERS = {
    "malformed_response": test_worker_malformed_response,
    "cold_start_timeout": test_worker_timeout,
    "worker_exit_on_status": test_real_child_process_exit,
    "worker_exit_on_asr": test_worker_exit,
    "worker_exit_on_translation": test_worker_exit,
    "worker_exit_on_tts": test_worker_exit,
    "empty_asr": test_asr_empty_response,
    "rate_channel_mismatch": test_wav_format_rejection,
    "missing_eos": test_translation_missing_eos,
    "silent_tts_wrong_actor": test_tts_silent_or_wrong_actor,
}


def execute(selected: set[str] | None = None, matrix: Path = MATRIX) -> dict:
    cases = matrix_cases(matrix)
    known = {row["id"] for row in cases}
    if selected is not None and not selected.issubset(known):
        raise ValueError("unknown fault IDs")
    rows = []
    for case in cases:
        ident = case["id"]
        result = {"id": ident, "slug": case["slug"],
                  "minimum_context": case["minimum_context"],
                  "status": "NOT_EXECUTED"}
        if selected is not None and ident not in selected:
            result["reason"] = "not_selected"
        elif case["slug"] in RUNNERS:
            try:
                evidence = RUNNERS[case["slug"]]()
            except Exception as exc:
                result.update({"status": "FAIL", "reason": type(exc).__name__})
            else:
                result.update({"status": "PASS", "observed": evidence,
                               "proof": "OFFLINE_CONTRACT_ONLY_NOT_NATIVE"})
        else:
            result["reason"] = "no_authorized_injector_implemented"
        rows.append(result)
    return {"schema": RECEIPT_SCHEMA, "cases": rows,
            "counts": {status: sum(row["status"] == status for row in rows)
                       for status in ("PASS", "FAIL", "NOT_EXECUTED")},
            "proof": "SOURCE_OFFLINE_FAULT_CONTRACT_NOT_ASR_MODEL_OR_MEETING"}


def main() -> int:
    parser = argparse.ArgumentParser(description="Offline audio fault contract checks")
    parser.add_argument("--ids", nargs="*")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    report = execute(set(args.ids) if args.ids is not None else None)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        # Never overwrite a prior run receipt.
        with args.output.open("x", encoding="utf-8") as handle:
            json.dump(report, handle, ensure_ascii=False, indent=2)
            handle.write("\n")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 1 if report["counts"]["FAIL"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
