"""Opt-in bridge to installed REA CLI for scoped developer investigations.

This is *not* a reverse-engineering engine, MCP server, installer, auto-auditor,
model provider, or product dependency. It never runs until --execute is given.
For REA's complete functionality, use its separately authorized MCP/CLI directly.
"""
from __future__ import annotations

import argparse
import io
import queue
import threading
import time
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_APPROVED_VERSION = "6.4.0"
MAX_DOCTOR_STDOUT_BYTES = 2 * 1024 * 1024
MAX_ANALYSIS_BYTES = 512 * 1024 * 1024
MAX_JSON_VALIDATION_BYTES = 32 * 1024 * 1024
VERSION = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][a-zA-Z0-9.-]+)?$")
IDENTIFIER = re.compile(r"^[a-z][a-z0-9_-]{0,63}$")


class ReaBridgeError(ValueError):
    pass


def _bounded_stdout(argv: list[str], output, limit: int, timeout: float) -> int:
    """Consume child stdout with an admission cap and deadline; no shell or full buffering."""
    try:
        proc = subprocess.Popen(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, shell=False)
    except OSError as exc:
        raise ReaBridgeError("REA process failed to launch") from exc
    done = threading.Event()
    exceeded = threading.Event()
    errors: list[BaseException] = []

    def copy() -> None:
        count = 0
        try:
            assert proc.stdout is not None
            while True:
                chunk = proc.stdout.read(64 * 1024)
                if not chunk:
                    break
                space = limit - count
                if len(chunk) > space:
                    if space > 0:
                        output.write(chunk[:space])
                    exceeded.set()
                    return
                output.write(chunk)
                count += len(chunk)
        except BaseException as exc:
            errors.append(exc)
        finally:
            done.set()

    reader = threading.Thread(target=copy, daemon=True, name="rea-bounded-stdout")
    reader.start()
    deadline = time.monotonic() + timeout
    try:
        while True:
            if exceeded.is_set():
                raise ReaBridgeError("REA output exceeded live byte limit")
            if errors:
                raise ReaBridgeError("REA output write failed") from errors[0]
            if proc.poll() is not None and done.is_set():
                break
            if time.monotonic() >= deadline:
                raise ReaBridgeError("REA command timed out")
            done.wait(0.05)
        reader.join(timeout=2)
        if reader.is_alive():
            raise ReaBridgeError("REA output pipe did not close")
        return proc.returncode if proc.returncode is not None else -1
    finally:
        if proc.poll() is None:
            proc.kill()
        try:
            proc.wait(timeout=4)
        except subprocess.TimeoutExpired as exc:
            raise ReaBridgeError("REA child cleanup unverified") from exc
        reader.join(timeout=2)
        if not reader.is_alive() and proc.stdout is not None:
            proc.stdout.close()


def _mcp_probe(command: list[str], version: str) -> dict:
    """Validate live MCP initialization and first-page tools/list, without any target."""
    env = os.environ.copy()
    env.pop("HOPPER_TARGET_PATH", None)
    env.pop("HOPPER_TARGET_KIND", None)
    try:
        proc = subprocess.Popen([*command, "mcp"], stdin=subprocess.PIPE,
                                stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                shell=False, env=env, bufsize=0)
    except OSError as exc:
        raise ReaBridgeError("REA MCP server failed to launch") from exc
    messages: queue.Queue = queue.Queue(maxsize=4)
    stop = threading.Event()
    deadline = time.monotonic() + 30

    def collect() -> None:
        try:
            assert proc.stdout is not None
            while not stop.is_set():
                chunk = proc.stdout.readline(12 * 1024 * 1024 + 1)
                if not chunk:
                    break
                if len(chunk) > 12 * 1024 * 1024:
                    messages.put(("error", "MCP frame exceeds maximum bytes"), timeout=1)
                    break
                try:
                    payload = json.loads(chunk)
                except (ValueError, UnicodeError):
                    messages.put(("error", "invalid MCP JSON frame"), timeout=1)
                    break
                if isinstance(payload, dict) and payload.get("id") in (1, 2):
                    messages.put(("result", payload), timeout=1)
        except (OSError, queue.Full):
            pass

    reader = threading.Thread(target=collect, daemon=True, name="rea-mcp-reader")
    reader.start()

    def send(value: dict) -> None:
        try:
            assert proc.stdin is not None
            proc.stdin.write((json.dumps(value) + "\n").encode("utf-8"))
            proc.stdin.flush()
        except (OSError, ValueError) as exc:
            raise ReaBridgeError("REA MCP send failed") from exc

    def receive(expected: int) -> dict:
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise ReaBridgeError("REA MCP handshake timed out")
            try:
                kind, payload = messages.get(timeout=min(remaining, 0.25))
            except queue.Empty:
                if proc.poll() is not None:
                    raise ReaBridgeError("REA MCP closed before handshake")
                continue
            if kind != "result":
                raise ReaBridgeError(str(payload))
            if not isinstance(payload, dict) or payload.get("id") != expected:
                raise ReaBridgeError("REA MCP request ID mismatch")
            if "error" in payload or not isinstance(payload.get("result"), dict):
                raise ReaBridgeError("REA MCP request failed")
            return payload["result"]

    try:
        send({"jsonrpc": "2.0", "id": 1, "method": "initialize",
              "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                         "clientInfo": {"name": "translateit-rea-check", "version": "1.0.0"}}})
        info = receive(1)
        if not isinstance(info.get("protocolVersion"), str):
            raise ReaBridgeError("REA MCP protocol negotiation missing")
        send({"jsonrpc": "2.0", "method": "notifications/initialized"})
        send({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}})
        listing = receive(2)
        tools = listing.get("tools")
        if not isinstance(tools, list) or not tools or any(
            not isinstance(item, dict) or not isinstance(item.get("name"), str)
            for item in tools
        ):
            raise ReaBridgeError("REA MCP returned invalid tool inventory")
        names = {item["name"] for item in tools}
        if "analyze_javascript_application" not in names:
            raise ReaBridgeError("REA MCP missing expected JavaScript analysis tool")
        return {"status": "REA_MCP_DISCOVERY_OBSERVED", "reaVersion": version,
                "negotiatedProtocol": info["protocolVersion"],
                "advertisedFirstPage": len(names),
                "paginationUnverified": bool(listing.get("nextCursor")),
                "providerRuntimeVerified": False, "targetAnalyzed": False,
                "meetingValidated": False, "proof": "MCP_INITIALIZE_TOOLS_LIST_ONLY"}
    finally:
        stop.set()
        if proc.stdin is not None:
            try:
                proc.stdin.close()
            except OSError:
                pass
        if proc.poll() is None:
            proc.kill()
        try:
            proc.wait(timeout=4)
        except subprocess.TimeoutExpired as exc:
            raise ReaBridgeError("REA MCP cleanup unverified") from exc
        reader.join(timeout=2)
        if not reader.is_alive() and proc.stdout is not None:
            proc.stdout.close()


def _command(script: str | None) -> list[str]:
    """Avoid executing Windows npm .cmd shims with untrusted file arguments."""
    if script is not None:
        path = Path(script)
        if not path.is_absolute() or not path.is_file():
            raise ReaBridgeError("--rea-script must be an existing absolute entrypoint")
        resolved = path.resolve()
        if resolved.name != "rea.mjs" or resolved.parent.name != "scripts":
            raise ReaBridgeError("--rea-script must select the installed REA scripts/rea.mjs")
        manifest = resolved.parent.parent / "package.json"
        try:
            package = json.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, ValueError) as exc:
            raise ReaBridgeError("installed REA package identity is unavailable") from exc
        if not isinstance(package, dict) or package.get("name") != "rea-agents":
            raise ReaBridgeError("entrypoint is not inside an identified rea-agents package")
        node = shutil.which("node")
        if node is None:
            raise ReaBridgeError("Node.js missing; REA requires an approved local Node runtime")
        return [node, str(resolved)]
    executable = shutil.which("rea")
    if executable is None:
        raise ReaBridgeError("REA CLI unavailable; use approved REA setup, no automatic install")
    if Path(executable).suffix.lower() in {".cmd", ".bat", ".ps1"}:
        raise ReaBridgeError(
            "Windows npm launcher detected; use --rea-script with installed rea-agents/scripts/rea.mjs"
        )
    return [executable]


def _doctor(command: list[str], version: str, client: str | None,
            provider: str | None) -> dict:
    args = [*command, "doctor"]
    for flag, value in (("--client", client), ("--provider", provider)):
        if value is not None:
            if not IDENTIFIER.fullmatch(value):
                raise ReaBridgeError(f"invalid {flag} identifier")
            args.extend((flag, value))
    args.append("--json")
    buffer = io.BytesIO()
    returncode = _bounded_stdout(args, buffer, MAX_DOCTOR_STDOUT_BYTES, 90)
    try:
        report = json.loads(buffer.getvalue())
    except (ValueError, UnicodeDecodeError) as exc:
        raise ReaBridgeError("REA doctor returned invalid JSON") from exc
    if not isinstance(report, dict):
        raise ReaBridgeError("REA doctor response is not an object")
    identity = report.get("identity")
    installed = identity.get("cli_package_version") if isinstance(identity, dict) else None
    if installed != version:
        raise ReaBridgeError("REA version differs from explicitly approved version")
    healthy = report.get("healthy")
    if not isinstance(healthy, bool) or not isinstance(report.get("scope_checks"), list):
        raise ReaBridgeError("REA doctor missing authoritative scope checks")
    # Exit code and scoped doctor status must agree; avoid reporting false success.
    if returncode != 0 or not healthy:
        return {"status": "REA_UNHEALTHY", "healthy": False, "version": installed,
                "scope": "selected" if (client or provider) else "full",
                "proof": "ACTUAL_REA_DOCTOR_ONLY", "meetingValidated": False}
    return {"status": "REA_DOCTOR_OBSERVED", "healthy": True, "version": installed,
            "scope": "selected" if (client or provider) else "full",
            "proof": "ACTUAL_REA_DOCTOR_ONLY", "meetingValidated": False}


def _target_and_output(target: str, output: str) -> tuple[Path, Path]:
    selected = Path(target)
    destination = Path(output)
    if not selected.is_absolute() or not selected.exists():
        raise ReaBridgeError("target must be an existing absolute local path")
    if not (selected.is_dir() or (selected.is_file() and selected.suffix.lower() == ".asar")):
        raise ReaBridgeError("static JavaScript target must be a directory or .asar file")
    if not destination.is_absolute() or destination.suffix.lower() != ".json":
        raise ReaBridgeError("output must be an absolute private .json destination")
    target_real, output_real = selected.resolve(), destination.resolve()
    if output_real.is_relative_to(ROOT.resolve()):
        raise ReaBridgeError("REA evidence cannot be stored inside the public source repository")
    if selected.is_dir() and output_real.is_relative_to(target_real):
        raise ReaBridgeError("REA result cannot be written into the inspected target")
    if destination.exists() or not destination.parent.is_dir():
        raise ReaBridgeError("private output must be absent and its parent must exist")
    return target_real, destination


def _analyze(command: list[str], target: Path, output: Path) -> dict:
    # Exclusive create: never overwrite evidence or follow a destination symlink.
    fd = os.open(str(output), os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(fd, "wb") as stream:
            returncode = _bounded_stdout(
                [*command, "analyze-javascript-application", str(target), "--json"],
                stream, MAX_ANALYSIS_BYTES, 600,
            )
        size = output.stat().st_size
        if returncode != 0 or size <= 0 or size > MAX_ANALYSIS_BYTES:
            raise ReaBridgeError("REA analysis failed, returned empty, or exceeded 512 MiB")
        # REA can emit hundreds of MB; never JSON-decode all such Evidence
        # in a wrapper. Validate small documents; large ones require the
        # upstream REA inspect-analysis-view/evidence tools for verification.
        syntax = "NOT_CHECKED_LARGE_JSON"
        if size <= MAX_JSON_VALIDATION_BYTES:
            try:
                with output.open("r", encoding="utf-8") as stream:
                    parsed = json.load(stream)
            except (OSError, UnicodeError, ValueError) as exc:
                raise ReaBridgeError("REA produced malformed JSON evidence") from exc
            if not isinstance(parsed, dict) or parsed.get("ok") is False or "error" in parsed:
                raise ReaBridgeError("REA returned an error or non-object analysis result")
            syntax = "VALID_JSON_DOCUMENT"
        digest = hashlib.sha256()
        with output.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
        # No private file path, transcript, source text or Evidence payload is
        # emitted to stdout. A syntactically valid document is not a proven
        # reconstruction, and large results retain explicit unknown syntax.
        return {"status": ("REA_ARTIFACT_CAPTURED_UNREVIEWED"
                           if syntax == "VALID_JSON_DOCUMENT"
                           else "REA_ARTIFACT_CAPTURED_SYNTAX_UNVERIFIED"),
                "bytes": size, "sha256": digest.hexdigest(),
                "jsonSyntax": syntax,
                "proof": "EXTERNAL_TOOL_COMPLETED_OUTPUT_NOT_SEMANTICALLY_VERIFIED",
                "meetingValidated": False}
    except BaseException:
        output.unlink(missing_ok=True)
        raise


def plan(kind: str, version: str, target: str | None) -> dict:
    return {
        "status": "PLANNED_NOT_EXECUTED", "operation": kind,
        "approvedVersion": version,
        "targetSelected": bool(target), "reaExecuted": False,
        "sourceAndRuntimeValidated": False,
        "approval": "EXPLICIT_LOCAL_EXECUTION_REQUIRED",
        "runbook": "docs/knowledge/operations/rea-investigation.md",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rea-script", help="Optional absolute installed scripts/rea.mjs entrypoint")
    parser.add_argument("--approved-version", default=DEFAULT_APPROVED_VERSION)
    commands = parser.add_subparsers(dest="operation", required=True)
    doctor = commands.add_parser("doctor", help="Check real REA readiness only when executed")
    doctor.add_argument("--client")
    doctor.add_argument("--provider")
    doctor.add_argument("--execute", action="store_true")
    probe = commands.add_parser("mcp-probe", help="MCP initialize + tools/list, no target")
    probe.add_argument("--execute", action="store_true")
    inspect = commands.add_parser("analyze-js", help="Inspect an authorized JS directory/ASAR")
    inspect.add_argument("--target", required=True)
    inspect.add_argument("--output", required=True)
    inspect.add_argument("--target-authorized", action="store_true")
    inspect.add_argument("--execute", action="store_true")
    args = parser.parse_args()
    try:
        if not VERSION.fullmatch(args.approved_version):
            raise ReaBridgeError("approved REA version must be an explicit semver")
        if not args.execute:
            result = plan(args.operation, args.approved_version, getattr(args, "target", None))
            print(json.dumps(result, indent=2))
            return 0
        if args.operation == "analyze-js":
            if not args.target_authorized:
                raise ReaBridgeError("explicit --target-authorized is required for target analysis")
            target, output = _target_and_output(args.target, args.output)
        command = _command(args.rea_script)
        ready = _doctor(command, args.approved_version,
                        args.client if args.operation == "doctor" else None,
                        args.provider if args.operation == "doctor" else None)
        if args.operation == "doctor":
            print(json.dumps(ready, indent=2))
            return 0 if ready["healthy"] else 3
        # Upstream explicitly documents that unscoped 'doctor' may be unhealthy
        # solely because unrelated optional native providers are unavailable,
        # while standalone static JavaScript analysis still works. The doctor
        # result verifies package identity but is NOT a static-JS admission gate.
        result = (_mcp_probe(command, ready["version"]) if args.operation == "mcp-probe"
                  else _analyze(command, target, output))
        print(json.dumps({**result, "reaVersion": ready["version"],
                          "generalDoctorHealthy": ready["healthy"]}, indent=2))
        return 0
    except (ReaBridgeError, OSError) as exc:
        print(json.dumps({"status": "BLOCKED_OR_UNKNOWN",
                          "reason": str(exc),
                          "reaExecutionProven": False,
                          "meetingValidated": False}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
