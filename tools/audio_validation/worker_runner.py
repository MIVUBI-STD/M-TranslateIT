"""Explicit, offline persistent-worker smoke for hashed public audio fixtures.

No GitHub CI, microphone, CPAL, Meeting session, cloud, or model downloads.
This exercises direct worker ASR/translation/optional selected voice only.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import time

from harness import (
    CORPUS, ROOT, SUITE, RESULT_SCHEMA, decode_wave, inspect,
    specs, fingerprint,
)

WORKER = ROOT / "EngineData/Backend/LocalWorker/WorkerRuntime/realtime_local_worker.py"
MAX_FRAME = 1_000_000


class WorkerTransportError(RuntimeError):
    pass


def pinned_source(expected: str) -> None:
    if not re.fullmatch(r"[a-f0-9]{40}", expected):
        raise ValueError("external exact 40-character Local SHA required")
    for command, actual in [
        (("rev-parse", "HEAD"), expected),
        (("branch", "--show-current"), "Local"),
        (("status", "--porcelain", "--untracked-files=no"), ""),
    ]:
        result = subprocess.run(["git", "-C", str(ROOT), *command],
                                capture_output=True, text=True, check=True, timeout=10)
        if result.stdout.strip() != actual:
            raise ValueError("Local checkout branch/SHA/clean tracked status mismatch")


class Worker:
    def __init__(self, python: Path, timeout: float):
        if not python.is_file() or not WORKER.is_file():
            raise ValueError("canonical pinned WorkerRuntime Python/entrypoint unavailable")
        if not 5 <= timeout <= 600:
            raise ValueError("worker request timeout outside 5–600 seconds")
        self.deadline = timeout
        self.count = 0
        self.messages: queue.Queue[bytes | None] = queue.Queue(maxsize=2)
        env = os.environ.copy()
        env.update({"HF_HUB_OFFLINE": "1", "TRANSFORMERS_OFFLINE": "1",
                    "HF_DATASETS_OFFLINE": "1", "TOKENIZERS_PARALLELISM": "false",
                    "PYTHONDONTWRITEBYTECODE": "1"})
        self.child = subprocess.Popen(
            [str(python), "-u", str(WORKER)], cwd=WORKER.parent,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
            env=env, shell=False,
        )
        self.reader = threading.Thread(target=self._read, daemon=True,
                                       name="audio-fixture-worker-reader")
        self.reader.start()

    def _read(self) -> None:
        assert self.child.stdout is not None
        while True:
            raw = self.child.stdout.readline(MAX_FRAME + 1)
            self.messages.put(raw if raw else None)
            if not raw or len(raw) > MAX_FRAME:
                return

    def invoke(self, command: str, **fields: object) -> dict:
        self.count += 1
        request_id = f"fixture-{self.count}"
        if self.child.poll() is not None:
            raise WorkerTransportError("worker exited before command")
        assert self.child.stdin is not None
        body = {"command": command, "request_id": request_id, **fields}
        try:
            self.child.stdin.write((json.dumps(body, ensure_ascii=False) + "\n").encode("utf-8"))
            self.child.stdin.flush()
            frame = self.messages.get(timeout=self.deadline)
        except (BrokenPipeError, OSError, queue.Empty) as exc:
            self.close()
            raise WorkerTransportError("persistent worker write/read timed out or closed") from exc
        if not frame or len(frame) > MAX_FRAME or not frame.endswith(b"\n"):
            self.close()
            raise WorkerTransportError("worker returned an empty/oversized/partial response")
        try:
            response = json.loads(frame)
        except (UnicodeError, json.JSONDecodeError) as exc:
            self.close()
            raise WorkerTransportError("invalid worker JSON frame") from exc
        if (not isinstance(response, dict) or response.get("request_id") != request_id
                or response.get("stage") not in (command, "local_realtime_worker_preflight")
                or type(response.get("ok")) is not bool):
            self.close()
            raise WorkerTransportError("worker request identity/stage/boolean mismatch")
        return response

    def close(self) -> None:
        if self.child.poll() is None:
            self.child.terminate()
            try:
                self.child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.child.kill()
                self.child.wait(timeout=5)
        for stream in (self.child.stdin, self.child.stdout):
            if stream is not None:
                stream.close()
        self.reader.join(timeout=2)


def stage_result(response: dict, name: str) -> dict:
    result = {"ok": response["ok"], "blocker": str(response.get("blocker", ""))[:160],
              "elapsed_ms": response.get("elapsed_ms")}
    if name == "asr":
        result["transcript_text"] = str(response.get("transcript_text", ""))[:4000]
    elif name == "translation":
        result.update({
            "direction_pair": response.get("direction_pair"),
            "complete": response.get("complete") is True,
            "finished_with_eos": response.get("finished_with_eos") is True,
            "translated_text": str(response.get("translated_text", ""))[:4000],
        })
    return result


def run(fixtures: Path, python: Path, expected_sha: str,
        selection: set[str], result_path: Path, voice: bool, timeout: float) -> dict:
    pinned_source(expected_sha)
    manifest = inspect(fixtures)
    suite, suite_id = specs()
    available = {x["id"] for x in suite}
    if not selection or not selection.issubset(available):
        raise ValueError("choose at least one known scenario ID")
    if result_path.exists() or result_path.is_symlink():
        raise ValueError("observation file must be new, never overwrite")
    fixture_rows = {x["id"]: x for x in manifest["fixtures"]}
    scratch_root = ROOT / "UserData/CacheData"
    scratch_root.mkdir(parents=True, exist_ok=True)
    records: list[dict] = []
    status: dict = {}
    stopped = False
    with tempfile.TemporaryDirectory(prefix="audio-fixture-run-", dir=scratch_root) as tmp:
        working = Path(tmp)
        worker = Worker(python, timeout)
        try:
            status = worker.invoke("status")
            asr_preload = worker.invoke("asr_preload")
            for case in suite:
                ident = case["id"]
                record = {"scenario_id": ident,
                          "audio_sha256": fixture_rows[ident]["sha256"], "stages": {}}
                records.append(record)
                if ident not in selection:
                    record["not_executed"] = "not_selected"
                    continue
                if not asr_preload.get("ok"):
                    record["not_executed"] = "asr_preload_failed"
                    continue
                audio = working / f"{ident}.wav"
                shutil.copyfile(fixtures/"samples"/f"{ident}.wav", audio)
                if fingerprint(audio) != record["audio_sha256"]:
                    raise ValueError("staged audio changed identity")
                started = time.monotonic()
                try:
                    asr = worker.invoke("transcribe", audio_path=str(audio),
                                        language=case["language"], beam_size=1, vad_filter=False)
                    record["stages"]["asr"] = stage_result(asr, "asr")
                    record["stages"]["asr"]["host_elapsed_ms"] = round((time.monotonic()-started)*1000)
                    if asr["ok"] and case["kind"] == "speech":
                        translated = worker.invoke(
                            "translate", text=str(asr.get("transcript_text", "")),
                            source_language=case["language"],
                            target_language="en" if case["language"] == "id" else "id",
                        )
                        record["stages"]["translation"] = stage_result(translated, "translation")
                        if voice and case["language"] == "id" and translated["ok"]:
                            preflight = worker.invoke("voice_actor_preflight")
                            record["stages"]["voice_preflight"] = stage_result(preflight, "voice")
                            if preflight["ok"]:
                                wav_out = working/f"{ident}-tts.wav"
                                produced = worker.invoke(
                                    "voice_actor_synthesize",
                                    text=str(translated.get("translated_text", "")),
                                    output_path=str(wav_out),
                                    expected_actor_token=preflight.get("actor_token", ""),
                                )
                                result = stage_result(produced, "tts")
                                if produced["ok"] and wav_out.is_file():
                                    _, rate = decode_wave(wav_out)
                                    result["wav_sha256"] = fingerprint(wav_out)
                                    with __import__("wave").open(str(wav_out), "rb") as w:
                                        result["duration_ms"] = round(w.getnframes()*1000/rate)
                                record["stages"]["tts"] = result
                except (WorkerTransportError, OSError) as exc:
                    record["transport_error"] = type(exc).__name__
                    stopped = True
                    break
        finally:
            worker.close()
    processed = {r["scenario_id"] for r in records}
    for case in suite:
        if case["id"] not in processed:
            records.append({"scenario_id": case["id"],
                            "audio_sha256": fixture_rows[case["id"]]["sha256"],
                            "stages": {}, "not_executed": "worker_transport_failed"})
    output = {
        "schema": RESULT_SCHEMA, "scenario_fingerprint": suite_id,
        "audio_fingerprint": manifest["audio_fingerprint"],
        "source_identity": {
            "local_sha": expected_sha, "context": "LOCAL_CODE_WORKER_ONLY",
            "worker_status_ok": status.get("ok") is True,
            "asr_model": status.get("asr_active_model_id", "unverified"),
            "translation_model": status.get("translation_model_id", "unverified"),
            "device": status.get("selected_device", "unverified"),
        },
        "results": records,
        "proof": "EXECUTED_DIRECT_WORKER_STAGES_NO_VAD_MICROPHONE_OR_RECEIVER",
        "transport_incomplete": stopped,
    }
    result_path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=result_path.parent, mode="w",
                                     encoding="utf-8", suffix=".tmp", delete=False) as handle:
        handle.write(json.dumps(output, ensure_ascii=False, indent=2)+"\n")
        staged = Path(handle.name)
    try:
        if result_path.exists():
            raise ValueError("observation destination appeared concurrently")
        os.replace(staged, result_path)
    finally:
        staged.unlink(missing_ok=True)
    return output


def main() -> int:
    parser = argparse.ArgumentParser(description="Explicit local AI worker audio smoke; never CI")
    parser.add_argument("--fixtures", type=Path, required=True)
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--ids", nargs="+", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--include-voice", action="store_true")
    args = parser.parse_args()
    result = run(args.fixtures, args.python, args.source_sha,
                 set(args.ids), args.output, args.include_voice, args.timeout)
    print(json.dumps({"cases": len(result["results"]), "proof": result["proof"],
                      "output": str(args.output)}, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError, WorkerTransportError, subprocess.CalledProcessError) as error:
        print(f"audio-runner: {type(error).__name__}: {error}", file=sys.stderr)
        raise SystemExit(2) from None
