"""Read-only offline preflight for deterministic public WAV fixture synthesis.

No files written, packages installed, online calls, models or CI invoked.
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

from harness import specs


def doctor(tts_exe: str) -> dict:
    executable = shutil.which(tts_exe)
    cases, suite_digest = specs()
    results = {"schema": "translateit.audio_generation_doctor.v1",
               "ok": False, "case_count": len(cases),
               "speech_cases": sum(row["kind"] == "speech" for row in cases),
               "signal_cases": sum(row["kind"] != "speech" for row in cases),
               "scenario_fingerprint": suite_digest,
               "generator": {"requested": tts_exe},
               "proof": "OFFLINE_SYNTHESIZER_AVAILABILITY_ONLY_NO_WAV_GENERATED"}
    if not executable:
        results["blocker"] = "offline_tts_executable_missing"
        return results
    results["generator"]["executable"] = executable
    version = subprocess.run([executable, "--version"], capture_output=True,
                             text=True, timeout=10, check=False)
    if version.returncode != 0:
        results["blocker"] = "offline_tts_version_query_failed"
        return results
    results["generator"]["version"] = version.stdout.strip()[:160]
    voices = {}
    for language in sorted({row["language"] for row in cases if row["kind"] == "speech"}):
        observed = subprocess.run([executable, f"--voices={language}"],
                                  capture_output=True, text=True, timeout=10, check=False)
        lines = observed.stdout.splitlines()
        voices[language] = (observed.returncode == 0 and len(lines) > 1
                            and any(language in line.split()[:3] for line in lines[1:]))
    results["generator"]["voices"] = voices
    results["ok"] = all(voices.values())
    if not results["ok"]:
        results["blocker"] = "required_offline_tts_voice_missing"
    return results


def main() -> int:
    parser = argparse.ArgumentParser(description="Read-only offline WAV generator preflight")
    parser.add_argument("--tts-exe", required=True)
    args = parser.parse_args()
    try:
        outcome = doctor(args.tts_exe)
    except (OSError, subprocess.TimeoutExpired) as exc:
        print(f"audio-generation-doctor: {type(exc).__name__}: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(outcome, ensure_ascii=False, indent=2))
    return 0 if outcome["ok"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
