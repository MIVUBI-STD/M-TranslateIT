"""Offline, deterministic audio fixture authority and strict, evidence-scoped grader.

No model, microphone, cloud, Windows audio route, or GitHub CI is launched by
validate/generate/inspect/score/compare. Corpus ground truth and WER remain
owned by tools/asr_quality/evaluate_asr_quality.py.
"""
from __future__ import annotations

import argparse
import array
import hashlib
import json
import math
import os
import random
import re
import shutil
import subprocess
import sys
import tempfile
import wave
from pathlib import Path

HERE = Path(__file__).resolve().parent
TOOLS = HERE.parent
ROOT = TOOLS.parent
sys.path.insert(0, str(TOOLS / "asr_quality"))
from evaluate_asr_quality import (  # noqa: E402
    load_corpus, corpus_fingerprint, invariant_result, word_error_rate, char_error_rate,
)

SUITE = HERE / "scenarios.json"
CORPUS = TOOLS / "asr_quality/corpus/asr_quality_v1.json"
RATE = 16000
LIMIT_S = 125
SUITE_SCHEMA = "translateit.audio_suite.v1"
FIXTURE_SCHEMA = "translateit.audio_fixtures.v1"
RESULT_SCHEMA = "translateit.audio_observations.v1"
SCORE_SCHEMA = "translateit.audio_scores.v1"
HASH = re.compile(r"[a-f0-9]{64}\Z")
SHA = re.compile(r"[a-f0-9]{40}\Z")
ID = re.compile(r"A[0-9]{2}\Z")
EXPECT = {"asr_mt_tts", "incoming", "vad_asr", "no_speech", "numeric_overflow"}
KIND = {"speech", "silence", "noise", "tone"}


def json_file(path: Path) -> dict:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("JSON root must be an object")
    return value


def digest(value: object) -> str:
    raw = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()


def fingerprint(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def specs(suite_path: Path = SUITE, corpus_path: Path = CORPUS) -> tuple[list[dict], str]:
    source = json_file(suite_path)
    if source.get("schema") != SUITE_SCHEMA or not isinstance(source.get("cases"), list):
        raise ValueError("invalid audio suite schema")
    corpus = load_corpus(corpus_path)
    reference = {item["id"]: item for item in corpus["cases"]}
    items, seen = [], set()
    for row in source["cases"]:
        if not isinstance(row, dict):
            raise ValueError("scenario must be an object")
        name = row.get("id")
        if not isinstance(name, str) or not ID.fullmatch(name) or name in seen:
            raise ValueError("duplicate or malformed audio scenario ID")
        seen.add(name)
        if row.get("kind") not in KIND or row.get("expect") not in EXPECT:
            raise ValueError(f"{name}: invalid signal kind/expectation")
        if row.get("language") not in ("id", "en"):
            raise ValueError(f"{name}: invalid language")
        if row["kind"] == "speech":
            source_id = row.get("case_id")
            literal = row.get("text")
            if bool(source_id) == bool(literal):
                raise ValueError(f"{name}: exactly one corpus ID or public literal required")
            if source_id and (source_id not in reference or row["language"] != "id"):
                raise ValueError(f"{name}: unknown Indonesian corpus case")
            words = reference[source_id]["reference"] if source_id else literal
            if not isinstance(words, str) or not (1 <= len(words) <= 2000):
                raise ValueError(f"{name}: invalid speech text")
            if type(row.get("max_wer")) not in (int, float) or not 0 <= row["max_wer"] <= 1:
                raise ValueError(f"{name}: ASR acceptance threshold required")
            if row["expect"] in ("no_speech", "numeric_overflow"):
                raise ValueError(f"{name}: speech cannot prove a non-speech scenario")
        else:
            words = ""
            if row.get("case_id") or row.get("text") or row["expect"] not in ("no_speech", "numeric_overflow"):
                raise ValueError(f"{name}: non-speech expectation/source mismatch")
            if type(row.get("duration_ms")) is not int:
                raise ValueError(f"{name}: non-speech duration required")
        for key, low, high in (
            ("duration_ms", 100, LIMIT_S*1000), ("prefix_ms", 0, 10000),
            ("suffix_ms", 0, 10000), ("comma_pause_ms", 0, 4000),
            ("rate_wpm", 90, 400), ("gain_db", -40, 24),
            ("snr_db", 1, 60), ("clip_amplitude", 0.05, 1.0),
            ("tone_hz", 30, RATE/2-1), ("tone_amplitude", 0, 0.95),
            ("noise_amplitude", 0, 0.95),
        ):
            val = row.get(key)
            if val is not None and (type(val) not in (int, float) or not math.isfinite(val) or not low <= val <= high):
                raise ValueError(f"{name}: {key} out of range")
        if row.get("comma_pause_ms") and "," not in words:
            raise ValueError(f"{name}: pause marker requires comma")
        items.append({**row, "reference": words})
    if not items:
        raise ValueError("audio suite may not be empty")
    return items, digest({"cases": items, "corpus": corpus_fingerprint(corpus)})


def decode_wave(path: Path) -> tuple[list[int], int]:
    if path.is_symlink() or not path.is_file() or path.stat().st_size > RATE*LIMIT_S*12:
        raise ValueError("WAV missing, linked, or too large")
    with wave.open(str(path), "rb") as audio:
        rate, count = audio.getframerate(), audio.getnframes()
        if (audio.getnchannels() != 1 or audio.getsampwidth() != 2
                or audio.getcomptype() != "NONE" or not 8000 <= rate <= 48000
                or not 0 < count <= rate * LIMIT_S):
            raise ValueError("expected bounded PCM16 mono WAV at 8–48 kHz")
        data = audio.readframes(count)
    if len(data) != count*2:
        raise ValueError("truncated WAV")
    pcm = array.array("h")
    pcm.frombytes(data)
    if sys.byteorder != "little":
        pcm.byteswap()
    return list(pcm), rate


def write_wave(path: Path, samples: list[int]) -> None:
    if not 0 < len(samples) <= RATE*LIMIT_S or any(not -32768 <= x <= 32767 for x in samples):
        raise ValueError("invalid PCM frame count or sample")
    pcm = array.array("h", samples)
    if sys.byteorder != "little":
        pcm.byteswap()
    with wave.open(str(path), "wb") as audio:
        audio.setnchannels(1)
        audio.setsampwidth(2)
        audio.setframerate(RATE)
        audio.writeframes(pcm.tobytes())


def resample(values: list[int], original: int) -> list[int]:
    if original == RATE:
        return values
    count = round(len(values)*RATE/original)
    output = []
    for i in range(count):
        idx, rem = divmod(i*original, RATE)
        if idx >= len(values)-1:
            output.append(values[-1])
        else:
            output.append(round((values[idx]*(RATE-rem)+values[idx+1]*rem)/RATE))
    return output


def generate_samples(row: dict, exe: str, temp: Path, seed: int) -> list[int]:
    kind = row["kind"]
    count = round(row.get("duration_ms", 0)*RATE/1000)
    rng = random.Random(f"translateit-audio-v1:{seed}:{row['id']}")
    if kind == "silence":
        return [0]*count
    if kind == "noise":
        sigma = row.get("noise_amplitude", .08)
        return [round(max(-1, min(1, rng.gauss(0, sigma)))*32767) for _ in range(count)]
    if kind == "tone":
        hz, amp = row.get("tone_hz", 220), row.get("tone_amplitude", .25)
        return [round(32767*amp*math.sin(2*math.pi*hz*i/RATE)) for i in range(count)]
    if not exe:
        raise ValueError("speech synthesis must be explicitly enabled with --tts-exe")
    text = row["reference"]
    parts = text.split(",", 1) if row.get("comma_pause_ms") else [text]
    samples = []
    for n, part in enumerate(parts):
        filename = temp / f"{row['id']}-{n}.wav"
        try:
            subprocess.run([exe, "-v", row["language"], "-s",
                            str(row.get("rate_wpm", 155)), "-w", str(filename), part.strip()],
                           shell=False, check=True, capture_output=True, timeout=45)
            pcm, rate = decode_wave(filename)
            samples.extend(resample(pcm, rate))
        finally:
            filename.unlink(missing_ok=True)
        if n+1 < len(parts):
            samples.extend([0]*round(row["comma_pause_ms"]*RATE/1000))
    samples = [0]*round(row.get("prefix_ms", 0)*RATE/1000) + samples
    samples.extend([0]*round(row.get("suffix_ms", 350)*RATE/1000))
    gain = 10**(row.get("gain_db", 0)/20)
    sigma = 0.0
    if "snr_db" in row:
        rms = math.sqrt(sum(x*x for x in samples)/max(1, len(samples)))
        sigma = rms/(10**(row["snr_db"]/20))
    limit = 32767*row.get("clip_amplitude", 1)
    return [round(max(-limit, min(limit, v*gain+rng.gauss(0, sigma)))) for v in samples]


def metrics(path: Path) -> dict:
    pcm, rate = decode_wave(path)
    return {"rate_hz": rate, "frames": len(pcm),
            "duration_ms": round(len(pcm)*1000/rate),
            "peak": round(max(abs(x) for x in pcm)/32768, 6),
            "rms": round(math.sqrt(sum(x*x for x in pcm)/len(pcm))/32768, 6)}


def fixture_digest(records: list[dict]) -> str:
    return digest([{"id": r["id"], "sha256": r["sha256"]} for r in records])


def generate(target: Path, tts: str, seed: int, suite_path: Path = SUITE,
             corpus_path: Path = CORPUS) -> dict:
    if target.exists() or target.is_symlink():
        raise ValueError("fixture directory already exists; never overwrite prior evidence")
    if not tts or not shutil.which(tts):
        raise ValueError("explicit offline espeak-compatible --tts-exe is required")
    items, suite_hash = specs(suite_path, corpus_path)
    version = subprocess.run([tts, "--version"], check=True, capture_output=True,
                             text=True, timeout=10).stdout.strip()[:160]
    target.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="audio-fixtures-", dir=target.parent) as tmpdir:
        temp = Path(tmpdir)
        (temp/"samples").mkdir()
        rows = []
        for item in items:
            wav = temp/"samples"/f"{item['id']}.wav"
            write_wave(wav, generate_samples(item, tts, temp, seed))
            rows.append({"id": item["id"], "file": f"samples/{item['id']}.wav",
                         "sha256": fingerprint(wav),
                         "reference_sha256": hashlib.sha256(item["reference"].encode()).hexdigest(),
                         "metrics": metrics(wav)})
        result = {"schema": FIXTURE_SCHEMA, "scenario_fingerprint": suite_hash,
                  "audio_fingerprint": fixture_digest(rows), "seed": seed,
                  "generator": {"name": "explicit-local-espeak", "version": version,
                                "resampler": "deterministic-linear-fixture-only"},
                  "fixtures": rows,
                  "proof": "GENERATED_WAV_ONLY_NOT_SPEECH_RECOGNITION_OR_NATIVE_PROOF"}
        (temp/"fixtures.json").write_text(json.dumps(result, indent=2, ensure_ascii=False)+"\n", encoding="utf-8")
        os.replace(temp, target)
    return result


def inspect(target: Path, suite_path: Path = SUITE,
            corpus_path: Path = CORPUS) -> dict:
    items, suite_hash = specs(suite_path, corpus_path)
    meta = json_file(target/"fixtures.json")
    if meta.get("schema") != FIXTURE_SCHEMA or meta.get("scenario_fingerprint") != suite_hash:
        raise ValueError("fixture identity mismatches canonical scenario/corpus")
    rows = meta.get("fixtures")
    if not isinstance(rows, list) or len(rows) != len(items):
        raise ValueError("fixture record coverage incomplete")
    expected = {r["id"]: r for r in items}
    if {r.get("id") for r in rows} != expected:
        raise ValueError("fixture IDs do not match scenario coverage")
    for row in rows:
        name = row["id"]
        wav = target/"samples"/f"{name}.wav"
        if (row.get("file") != f"samples/{name}.wav"
                or not HASH.fullmatch(str(row.get("sha256", "")))
                or row.get("reference_sha256") != hashlib.sha256(expected[name]["reference"].encode()).hexdigest()
                or fingerprint(wav) != row["sha256"]
                or metrics(wav) != row.get("metrics")):
            raise ValueError(f"{name}: fixture hash/reference/WAV differs")
    if meta.get("audio_fingerprint") != fixture_digest(rows):
        raise ValueError("fixture set fingerprint differs")
    return meta


def score(target: Path, observations: Path, suite_path: Path = SUITE,
          corpus_path: Path = CORPUS) -> dict:
    fixture = inspect(target, suite_path, corpus_path)
    cases, suite_hash = specs(suite_path, corpus_path)
    corpus = {x["id"]: x for x in load_corpus(corpus_path)["cases"]}
    proof = json_file(observations)
    identity = proof.get("source_identity")
    if (proof.get("schema") != RESULT_SCHEMA or proof.get("scenario_fingerprint") != suite_hash
            or proof.get("audio_fingerprint") != fixture["audio_fingerprint"]
            or not isinstance(identity, dict) or not SHA.fullmatch(str(identity.get("local_sha", "")))):
        raise ValueError("observation exact source/scenario/audio identity missing")
    rows = proof.get("results")
    if not isinstance(rows, list) or len(rows) != len(cases):
        raise ValueError("observations need one row per case, including unexecuted")
    by_id = {}
    for row in rows:
        if not isinstance(row, dict) or row.get("scenario_id") in by_id:
            raise ValueError("duplicate/malformed observation")
        by_id[row["scenario_id"]] = row
    wavs = {r["id"]: r["sha256"] for r in fixture["fixtures"]}
    results = []
    for case in cases:
        name = case["id"]
        row = by_id.get(name)
        if not row or row.get("audio_sha256") != wavs[name]:
            raise ValueError(f"{name}: missing observation or mismatched audio hash")
        stages = row.get("stages", {})
        if not isinstance(stages, dict):
            raise ValueError(f"{name}: stages must be an object")
        required = {
            "asr_mt_tts": ["asr", "translation", "tts"],
            "incoming": ["asr", "translation"],
            "vad_asr": ["vad", "asr"],
            "no_speech": ["vad"],
            "numeric_overflow": ["vad"],
        }[case["expect"]]
        missing = [x for x in required if not isinstance(stages.get(x), dict)
                   or type(stages[x].get("ok")) is not bool]
        failed = [x for x in required if x not in missing and stages[x]["ok"] is False]
        status = "UNVERIFIED" if missing else "FAIL" if failed else "PASS"
        detail = {"missing": missing, "failed": failed}
        asr = stages.get("asr")
        if case["kind"] == "speech" and isinstance(asr, dict) and asr.get("ok") is True:
            transcript = asr.get("transcript_text")
            if not isinstance(transcript, str) or not transcript.strip():
                status, detail["cause"] = "FAIL", "ASR empty success"
            else:
                wer = word_error_rate(case["reference"], transcript)
                detail["wer"] = round(wer, 4)
                detail["cer"] = round(char_error_rate(case["reference"], transcript), 4)
                if case.get("case_id"):
                    critical = invariant_result(corpus[case["case_id"]], transcript)
                    detail["critical"] = critical
                    if not critical["critical_pass"]:
                        status = "FAIL"
                if wer > case["max_wer"]:
                    status, detail["cause"] = "FAIL", "ASR WER over threshold"
        trans = stages.get("translation")
        if isinstance(trans, dict) and trans.get("ok") is True:
            if (trans.get("direction_pair") != ("id->en" if case["language"] == "id" else "en->id")
                    or trans.get("complete") is not True or trans.get("finished_with_eos") is not True
                    or not str(trans.get("translated_text", "")).strip()):
                status, detail["cause"] = "FAIL", "incomplete/wrong-direction translation"
        tts = stages.get("tts")
        if isinstance(tts, dict) and tts.get("ok") is True:
            if (not HASH.fullmatch(str(tts.get("wav_sha256", "")))
                    or type(tts.get("duration_ms")) not in (int, float)
                    or tts["duration_ms"] <= 0):
                status, detail["cause"] = "FAIL", "TTS WAV receipt missing"
        vad = stages.get("vad")
        if isinstance(vad, dict) and vad.get("ok") is True:
            n = vad.get("finalized_count")
            if type(n) is not int or n < 0:
                status, detail["cause"] = "FAIL", "VAD finalized count missing"
            elif case["expect"] == "no_speech" and n != 0:
                status, detail["cause"] = "FAIL", "false speech finalization"
            elif case["expect"] == "numeric_overflow" and (
                n != 0 or type(vad.get("overflow_count")) is not int or vad["overflow_count"] < 1
            ):
                status, detail["cause"] = "FAIL", "numeric stress count unproven"
            elif case["expect"] == "vad_asr" and (n < 1 or vad.get("writer_ok") is not True):
                status, detail["cause"] = "FAIL", "VAD never reached verified WAV writer"
        if case["kind"] != "speech" and isinstance(asr, dict) and asr.get("transcript_text"):
            status, detail["cause"] = "FAIL", "unexpected non-speech transcription"
        if case["expect"] == "numeric_overflow":
            detail["boundary"] = "TONE_ONLY_NOT_LONG_SPEECH_ACCEPTANCE"
        results.append({"id": name, "status": status, "checks": detail})
    return {"schema": SCORE_SCHEMA, "scenario_fingerprint": suite_hash,
            "audio_fingerprint": fixture["audio_fingerprint"], "source_identity": identity,
            "counts": {name: sum(r["status"] == name for r in results)
                       for name in ("PASS", "FAIL", "UNVERIFIED")},
            "results": results,
            "proof": "OBSERVED_STAGES_ONLY_NOT_DEVICE_OR_MEETING_RECEIVER_PROOF"}


def compare(base: dict, candidate: dict) -> dict:
    if (base.get("schema") != SCORE_SCHEMA or candidate.get("schema") != SCORE_SCHEMA
            or base.get("scenario_fingerprint") != candidate.get("scenario_fingerprint")
            or base.get("audio_fingerprint") != candidate.get("audio_fingerprint")):
        raise ValueError("baseline/candidate require identical audio bytes and scenario identity")
    before = {r["id"]: r for r in base["results"]}
    after = {r["id"]: r for r in candidate["results"]}
    if before.keys() != after.keys():
        raise ValueError("baseline/candidate coverage differs")
    return {"baseline": base["source_identity"], "candidate": candidate["source_identity"],
            "regressions": sorted(k for k in before if before[k]["status"] == "PASS" and after[k]["status"] != "PASS"),
            "recoveries": sorted(k for k in before if before[k]["status"] != "PASS" and after[k]["status"] == "PASS"),
            "audio_fingerprint": base["audio_fingerprint"],
            "note": "A matching fixture set is necessary, not sufficient, for native/quality proof"}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Offline audio fixtures/evidence; no cloud or CI")
    parser.add_argument("command", choices=("validate", "generate", "inspect", "score", "compare"))
    parser.add_argument("--suite", type=Path, default=SUITE)
    parser.add_argument("--corpus", type=Path, default=CORPUS)
    parser.add_argument("--fixtures", type=Path)
    parser.add_argument("--tts-exe")
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--observations", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--candidate", type=Path)
    args = parser.parse_args(argv)
    if args.command == "validate":
        items, identity = specs(args.suite, args.corpus)
        answer = {"ok": True, "case_count": len(items), "scenario_fingerprint": identity,
                  "proof": "SPECIFICATION_ONLY"}
    elif args.command == "generate":
        if args.fixtures is None:
            parser.error("generate requires --fixtures")
        answer = generate(args.fixtures, args.tts_exe or "", args.seed, args.suite, args.corpus)
    elif args.command == "inspect":
        if args.fixtures is None:
            parser.error("inspect requires --fixtures")
        answer = inspect(args.fixtures, args.suite, args.corpus)
    elif args.command == "score":
        if args.fixtures is None or args.observations is None:
            parser.error("score requires --fixtures and --observations")
        answer = score(args.fixtures, args.observations, args.suite, args.corpus)
    else:
        if args.baseline is None or args.candidate is None:
            parser.error("compare requires --baseline and --candidate")
        answer = compare(json_file(args.baseline), json_file(args.candidate))
    print(json.dumps(answer, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as exc:
        print(f"audio-fixtures: {type(exc).__name__}: {exc}", file=sys.stderr)
        raise SystemExit(2) from None
