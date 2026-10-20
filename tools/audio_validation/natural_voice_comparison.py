"""Explicit opt-in Azure neural Indonesian comparison fixture generator.

Never run during CI or import. Uses only PUBLIC approved scripts, refuses
overwrites, requires a locally provided Speech key and region, bounds 10
requests, and records original + canonical WAV hashes separately.
This is fixture preparation, not TranslateIT cloud inference.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile
from urllib import request, error
from xml.sax.saxutils import escape

from harness import RATE, decode_wave, resample, specs, write_wave, fingerprint, metrics

SCHEMA = "translateit.natural_voice_comparison.v1"
CASES = ("A01", "A03", "A04", "A06", "A08")
VOICES = ("id-ID-ArdiNeural", "id-ID-GadisNeural")
REGION = re.compile(r"[a-z0-9-]{2,32}\Z")
MAX_AUDIO_BYTES = 4_000_000


def ssml(text: str, voice: str) -> bytes:
    if voice not in VOICES or not text or len(text) > 1600:
        raise ValueError("unsupported voice or script")
    body = (f'<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" '
            f'xml:lang="id-ID"><voice name="{voice}">{escape(text)}</voice></speak>')
    return body.encode("utf-8")


def synthesize(text: str, voice: str, *, region: str, key: str) -> bytes:
    if not REGION.fullmatch(region) or not key:
        raise ValueError("valid Azure region and explicit Speech key required")
    url = f"https://{region}.tts.speech.microsoft.com/cognitiveservices/v1"
    req = request.Request(
        url,
        data=ssml(text, voice),
        method="POST",
        headers={
            "Ocp-Apim-Subscription-Key": key,
            "Content-Type": "application/ssml+xml",
            "X-Microsoft-OutputFormat": "riff-24khz-16bit-mono-pcm",
            "User-Agent": "TranslateIT-Offline-Fixture-Preparation",
        },
    )
    try:
        with request.urlopen(req, timeout=40) as response:
            if response.status != 200:
                raise ValueError("cloud synthesizer did not return HTTP 200")
            content = response.read(MAX_AUDIO_BYTES + 1)
    except error.HTTPError as exc:
        raise ValueError(f"cloud synthesis HTTP {exc.code}") from None
    except error.URLError:
        raise ValueError("cloud synthesis transport unavailable") from None
    if not content or len(content) > MAX_AUDIO_BYTES:
        raise ValueError("cloud WAV missing or oversized")
    return content


def plan() -> dict:
    cases, fingerprint_value = specs()
    by_id = {row["id"]: row for row in cases}
    selected = []
    for case_id in CASES:
        row = by_id[case_id]
        if row["kind"] != "speech" or row["language"] != "id":
            raise ValueError("comparison scripts must be public Indonesian speech")
        selected.append({"id": case_id, "reference": row["reference"]})
    return {"schema": SCHEMA, "scenario_fingerprint": fingerprint_value,
            "voices": list(VOICES), "samples": selected, "requests": len(selected)*len(VOICES),
            "proof": "REQUEST_PLAN_ONLY_NO_CLOUD_CALLS"}


def generate(destination: Path, *, region: str, key: str) -> dict:
    if destination.exists() or destination.is_symlink():
        raise ValueError("comparison destination already exists")
    data = plan()
    if not REGION.fullmatch(region) or not key:
        raise ValueError("explicit valid Azure region/key required")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=destination.parent, prefix="natural-voices-") as root:
        tmp = Path(root)
        samples = tmp / "samples"
        samples.mkdir()
        outputs = []
        for voice in VOICES:
            for row in data["samples"]:
                name = f'{row["id"]}-{voice}.wav'
                native = samples / ("native-" + name)
                canonical = samples / name
                raw = synthesize(row["reference"], voice, region=region, key=key)
                native.write_bytes(raw)
                pcm, rate = decode_wave(native)
                if rate != 24000:
                    raise ValueError("Azure PCM rate changed; refuse hidden conversion")
                write_wave(canonical, resample(pcm, rate))
                checked = metrics(canonical)
                if checked["rms"] <= 0.0005 or checked["peak"] <= 0.01:
                    raise ValueError("neural voice returned silent or invalid WAV")
                outputs.append({
                    "id": row["id"], "voice": voice, "file": "samples/"+name,
                    "reference_sha256": hashlib.sha256(row["reference"].encode()).hexdigest(),
                    "native_sha256": fingerprint(native),
                    "sha256": fingerprint(canonical), "metrics": checked,
                })
                native.unlink()
        evidence = {**data, "region": region, "files": outputs,
                    "proof": "GENERATED_PUBLIC_NEURAL_WAV_ONLY_NOT_ASR_OR_DEVICE_PASS"}
        (tmp / "comparison.json").write_text(
            json.dumps(evidence, ensure_ascii=False, indent=2)+"\n", encoding="utf-8"
        )
        if destination.exists() or destination.is_symlink():
            raise ValueError("destination appeared while generating")
        tmp.rename(destination)
    return evidence


def main() -> int:
    parser = argparse.ArgumentParser(description="Opt-in public Indonesian neural TTS fixture comparison")
    parser.add_argument("command", choices=("plan", "generate"))
    parser.add_argument("--destination", type=Path)
    parser.add_argument("--region")
    args = parser.parse_args()
    if args.command == "plan":
        outcome = plan()
    else:
        if args.destination is None:
            parser.error("generate requires --destination")
        key = os.environ.get("AZURE_SPEECH_KEY", "")
        region = args.region or os.environ.get("AZURE_SPEECH_REGION", "")
        if not key:
            parser.error("AZURE_SPEECH_KEY environment variable is required for explicit paid requests")
        outcome = generate(args.destination, region=region, key=key)
    print(json.dumps(outcome, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError) as exc:
        raise SystemExit(f"natural-audio: {type(exc).__name__}: {exc}") from None
