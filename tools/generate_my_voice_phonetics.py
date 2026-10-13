#!/usr/bin/env python3
"""Generate optional CMUdict *text* coverage hints for the My Voice corpus.

Developer-only, offline, no network, no model changes. The app does not need
Python, a dictionary download or G2P at runtime for recommendations.

Usage:
  python tools/generate_my_voice_phonetics.py \
    --dictionary /absolute/path/to/cmudict.dict --check

Source dictionary reference: cmusphinx/cmudict Git blob
2c0411740cce3e2026a80b90b650d5f6a7258164.
The dictionary license/attribution is documented in the My Voice architecture.
This generates derived bitsets, not the dictionary source.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
SOURCES = ROOT / "EngineData/Frontend/RustApp/src-tauri/src/commands/voice_lab_recording"
OUTPUT = SOURCES / "guided_phonetic_inventory.rs"
FILES = (
    "guided_lines.rs",
    "guided_corpus_short.rs",
    "guided_corpus_natural.rs",
    "guided_corpus_detailed.rs",
)
DICT_LINE = re.compile(r"^(\S+)\s+((?:[A-Z]+\d?\s*)+)$")
PROMPT = re.compile(r'^\s*\((\d+),\s*("(?:[^"\\]|\\.)*")(?:,\s*"[^"]+",\s*"[^"]+")?\),?$')
WORD = re.compile(r"[a-z]+(?:'[a-z]+)*", re.IGNORECASE)
VARIANT = re.compile(r"\(\d+\)$")
STRESS = re.compile(r"[012]$")


def corpus():
    entries = []
    for name in FILES:
        for line in (SOURCES / name).read_text(encoding="utf-8").splitlines():
            match = PROMPT.fullmatch(line)
            if match:
                entries.append((int(match[1]), json.loads(match[2])))
    if len(entries) != 600 or any(item[0] != position for position, item in enumerate(entries, 1)):
        raise ValueError("Corpus must contain exact sequential prompt IDs 1-600; no generation")
    return entries


def dictionary(path: Path):
    lookup = {}
    labels = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        match = DICT_LINE.fullmatch(line)
        if not match:
            continue
        word = VARIANT.sub("", match[1]).lower()
        if word in lookup:
            continue
        phones = tuple(STRESS.sub("", x) for x in match[2].split())
        lookup[word] = phones
        labels.update(phones)
    if len(labels) != 39 or len(lookup) < 100000:
        raise ValueError("Unexpected CMUdict inventory: verify pinned dictionary input")
    return lookup, sorted(labels)


def fnv64(text: str):
    v = 0xCBF29CE484222325
    for byte in text.encode("utf-8"):
        v = ((v ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f"0x{v:016x}"


def bitset(keys: set[str], labels: list[str]):
    value = 0
    for index, key in enumerate(labels):
        if key in keys:
            value |= 1 << index
    return f"0x{value:016x}"


def generate(lexicon, phones, entries):
    observed = []
    frequencies = Counter()
    for identifier, sentence in entries:
        words = [word.lower() for word in WORD.findall(sentence)]
        found = 0
        seen_phones = set()
        pairs = set()
        for word in words:
            sequence = lexicon.get(word)
            if sequence is None:
                continue
            found += 1
            seen_phones.update(sequence)
            pairs.update(a + "-" + b for a, b in zip(sequence, sequence[1:]))
        frequencies.update(pairs)  # One mention per sentence, not duration.
        observed.append((identifier, sentence, words, found, seen_phones, pairs))

    eligible = sorted(
        ((pair, count) for pair, count in frequencies.items() if 8 <= count <= 145),
        key=lambda entry: (-entry[1], entry[0]),
    )
    buckets = defaultdict(list)
    for pair, _ in eligible:
        buckets[pair.split("-", 1)[0]].append(pair)
    selected = []
    for round_number in range(4):
        if len(selected) >= 64:
            break
        for phone in phones:
            items = buckets[phone]
            if round_number < len(items):
                pair = items[round_number]
                if pair not in selected:
                    selected.append(pair)
            if len(selected) >= 64:
                break
    for pair, _ in eligible:
        if len(selected) >= 64:
            break
        if pair not in selected:
            selected.append(pair)
    if len(selected) != 64:
        raise ValueError("Insufficient representative within-word pair inventory")

    lines = [
        "// GENERATED ADVISORY PRONUNCIATION HINTS. Not recorded audio or quality scores.",
        "// CMUdict source GitHub blob 2c0411740cce3e2026a80b90b650d5f6a7258164.",
        "// First US-English dictionary variant. Unknown pronunciations are omitted.",
        "// Regenerate using tools/generate_my_voice_phonetics.py. No dictionary text bundled.",
        "// Each tuple: text UTF-8 FNV64, ARPABET phone mask, selected within-word pair mask,",
        "// and per-line CMUdict word-match ratio / 1000. Index 0 corresponds to ID 1.",
        "#[allow(dead_code)]",
        f"pub(super) const PHONE_LABELS: [&str; 39] = {json.dumps(phones, separators=(',', ':'))};",
        "#[allow(dead_code)]",
        f"pub(super) const PAIR_LABELS: [&str; 64] = {json.dumps(selected, separators=(',', ':'))};",
        "pub(super) const FEATURES: &[(u64, u64, u64, u16)] = &[",
    ]
    for identifier, sentence, words, found, seen_phones, pairs in observed:
        # Match Rust/JS generation with nearest-integer rule. All ratios are
        # informational; an unknown proper name is not an "incorrect accent".
        ratio = int(found * 1000 / max(1, len(words)) + 0.5)
        lines.append(
            f"    ({fnv64(sentence)}, {bitset(seen_phones, phones)}, "
            f"{bitset(pairs, selected)}, {ratio}), // line {identifier}"
        )
    return "\n".join([*lines, "];", ""])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dictionary", required=True, type=Path)
    parser.add_argument("--check", action="store_true", help="Compare only; do not rewrite generated source")
    arguments = parser.parse_args()
    if not arguments.dictionary.is_file():
        parser.error("Provide an already-installed CMUdict file; do not download automatically")
    lexicon, phones = dictionary(arguments.dictionary)
    generated = generate(lexicon, phones, corpus())
    if arguments.check:
        if OUTPUT.read_text(encoding="utf-8") != generated:
            raise SystemExit("My Voice phonetic hints are stale; regenerate from the reviewed dictionary")
        print("My Voice phonetic hint inventory matches the 600 source prompts")
        return
    OUTPUT.write_text(generated, encoding="utf-8")
    print("Generated My Voice phonetic hints from the provided offline CMUdict file")


if __name__ == "__main__":
    main()
