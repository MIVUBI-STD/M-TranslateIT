"""Opt-in bridge to installed REA CLI for scoped developer investigations.

This is *not* a reverse-engineering engine, MCP server, installer, auto-auditor,
model provider, or product dependency. It never runs until --execute is given.
For REA's complete functionality, use its separately authorized MCP/CLI directly.
"""
from __future__ import annotations

import argparse
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
    try:
        result = subprocess.run(
            args, capture_output=True, timeout=90, check=False
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise ReaBridgeError("REA doctor unavailable or timed out") from exc
    if len(result.stdout) > MAX_DOCTOR_STDOUT_BYTES:
        raise ReaBridgeError("REA doctor output exceeds bounded limit")
    try:
        report = json.loads(result.stdout)
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
    if result.returncode != 0 or not healthy:
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
            try:
                completed = subprocess.run(
                    [*command, "analyze-javascript-application", str(target), "--json"],
                    stdout=stream, stderr=subprocess.DEVNULL, timeout=600, check=False
                )
            except (OSError, subprocess.TimeoutExpired) as exc:
                raise ReaBridgeError("REA static analysis could not complete") from exc
        size = output.stat().st_size
        if completed.returncode != 0 or size <= 0 or size > MAX_ANALYSIS_BYTES:
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
        result = _analyze(command, target, output)
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
