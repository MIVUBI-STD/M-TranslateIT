"""Admit committed, data-only TranslateIT Skills without executing their contents.

This is a conservative static supply-chain gate. It neither proves prompt
safety nor authorizes a new skill, external network use, dependency installation,
secrets, runtime mutation, or agent model behavior. Root AGENTS.md and the
canonical registry retain mode, scope and skill identity authority.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import stat
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SKILLS = Path(".agents/skills")
REGISTRY = Path(".agents/skill-registry.json")
DATA_EXTENSIONS = frozenset({".md", ".txt", ".json", ".png", ".jpg", ".jpeg", ".webp"})
MAX_SKILL_FILES = 64
MAX_SKILL_ENTRIES = 128
MAX_PACKAGE_DEPTH = 8
MAX_FILE_BYTES = 512_000
MAX_PACKAGE_BYTES = 2_000_000
MAX_MARKDOWN_BYTES = 48_000
MAX_TEXT_BYTES = 128_000
SAFE_NAME = re.compile(r"^[a-zA-Z0-9_.-]+$")
HEADER = re.compile(r"\A---\r?\n(.*?)\r?\n---\r?\n", re.DOTALL)
HEADING = re.compile(r"(?m)^#{2,3} +(.+?) *$")
SUSPICIOUS = (
    ("download-and-execute shell pipeline", re.compile(
        r"\b(?:curl|wget)\b[^\n]{0,200}\|\s*(?:sudo\s+)?(?:sh|bash|zsh|python(?:3)?)\b", re.I)),
    ("decoded payload executed via pipe", re.compile(
        r"\b(?:base64\s+(?:-d|--decode)|atob\s*\()[^\n]{0,160}\|\s*(?:sh|bash|zsh|node|python(?:3)?)\b", re.I)),
    ("PowerShell download-and-execute pipeline", re.compile(
        r"\b(?:irm|iwr|Invoke-RestMethod|Invoke-WebRequest)\b[^\n]{0,160}\|\s*(?:iex|Invoke-Expression)\b", re.I)),
    ("explicit instruction-priority override", re.compile(
        r"\bignore\s+(?:all\s+)?(?:previous|prior|system|developer)\s+instructions\b", re.I)),
    ("forged model role delimiter", re.compile(
        r"<\|(?:im_start|im_end)\|>|\[(?:im_start|start_header_id)\]", re.I)),
    ("role-injection marker", re.compile(
        r"(?im)^\s*(?:system|developer)\s*:\s*(?:override|ignore|supersede)\b")),
)
SPECIALIST_SECTIONS = ("## Owns", "## Rules", "## Procedure", "## Proof")
BRIEF_SECTIONS = ("## Required contract", "## Procedure", "## Completion")


def _read_registry(root: Path) -> list[dict[str, Any]]:
    payload = json.loads((root / REGISTRY).read_text(encoding="utf-8"))
    if payload.get("schemaVersion") != 1 or not isinstance(payload.get("skills"), list):
        raise ValueError("invalid canonical skill registry")
    items = payload["skills"]
    if not items or any(not isinstance(item, dict) for item in items):
        raise ValueError("invalid skill registry entries")
    names = [item.get("id") for item in items]
    if (any(not isinstance(name, str) for name in names)
            or len(set(names)) != len(items)):
        raise ValueError("invalid or duplicated skill identities")
    for item in items:
        name = item["id"]
        if (not isinstance(name, str) or not re.fullmatch(r"[a-z][a-z0-9-]{1,70}", name)
                or item.get("kind") not in {"planning", "specialist"}):
            raise ValueError("unregistered or unsafe skill identity/classification")
    return items


def _validate_markdown(path: Path, content: str, identity: str,
                       kind: str) -> list[str]:
    problems: list[str] = []
    found = HEADER.match(content)
    if not found:
        return ["missing bounded SKILL.md frontmatter"]
    metadata: dict[str, str] = {}
    for line in found.group(1).splitlines():
        if ":" not in line:
            problems.append("malformed SKILL.md frontmatter")
            continue
        name, value = line.split(":", 1)
        if name not in {"name", "description"} or name in metadata or not value.strip():
            problems.append("unsupported, duplicated or empty SKILL.md metadata")
        else:
            metadata[name] = value.strip()
    if set(metadata) != {"name", "description"} or metadata.get("name") != identity:
        problems.append("SKILL.md frontmatter does not match registered identity")
    description = metadata.get("description", "")
    if not 80 <= len(description) <= 600:
        problems.append("SKILL.md description must be bounded and usable for routing")
    if kind == "specialist" and not re.search(r"\b(?:do not|not for|outside|exclude)\b",
                                              description, re.I):
        problems.append("specialist description lacks a negative activation boundary")
    for heading in BRIEF_SECTIONS if kind == "planning" else SPECIALIST_SECTIONS:
        if heading not in content:
            problems.append("missing canonical procedure/proof section: " + heading)
    return problems


def _is_valid_image(ext: str, data: bytes) -> bool:
    if ext == ".png":
        return data.startswith(b"\x89PNG\r\n\x1a\n")
    if ext in (".jpg", ".jpeg"):
        return data.startswith(b"\xff\xd8\xff")
    if ext == ".webp":
        return len(data) >= 12 and data[:4] == b"RIFF" and data[8:12] == b"WEBP"
    return False


def admit_skills(root: Path = ROOT) -> list[str]:
    """Inspect current committed packages only. Never import/execute skill files."""
    problems: list[str] = []
    root = root.resolve()
    # A linked root bypasses the directory and package checks.
    for parent in (root / ".agents", root / SKILLS):
        if parent.is_symlink() or not parent.is_dir():
            return ["skill admission root is missing or symlinked: " + str(parent)]
    registry = _read_registry(root)
    expected = {item["id"]: item for item in registry}
    skill_root = root / SKILLS
    skill_entries = list(os.scandir(skill_root))
    actual = set()
    for item in skill_entries:
        if item.is_symlink() or not item.is_dir(follow_symlinks=False):
            problems.append("unreviewed skill-root entry: " + item.name)
        else:
            actual.add(item.name)
    if actual != set(expected):
        problems.append("skill package folders differ from canonical inventory")

    count, entries_seen, total = 0, 0, 0
    for identity, item in sorted(expected.items()):
        directory = skill_root / identity
        if not directory.is_dir() or directory.is_symlink():
            problems.append(identity + ": skill package missing or linked")
            continue
        seen_skill = False
        pending = [directory]
        while pending:
            current = pending.pop()
            for entry in os.scandir(current):
                entries_seen += 1
                if entries_seen > MAX_SKILL_ENTRIES:
                    problems.append("skill packages exceed bounded entry budget")
                    return sorted(set(problems))
                relative = Path(entry.path).relative_to(skill_root).as_posix()
                if len(Path(entry.path).relative_to(directory).parts) > MAX_PACKAGE_DEPTH:
                    problems.append("nested skill package exceeds depth budget: " + relative)
                    continue
                if not SAFE_NAME.fullmatch(entry.name) or entry.name.startswith("."):
                    problems.append("unsafe skill path: " + relative)
                    continue
                if entry.is_symlink():
                    problems.append("symlink inside skill package: " + relative)
                    continue
                if entry.is_dir(follow_symlinks=False):
                    pending.append(Path(entry.path))
                    continue
                if not entry.is_file(follow_symlinks=False):
                    problems.append("unsupported filesystem entry in skill: " + relative)
                    continue
                count += 1
                if count > MAX_SKILL_FILES:
                    problems.append("too many skill files")
                    return sorted(set(problems))
                metadata = entry.stat(follow_symlinks=False)
                total += metadata.st_size
                if total > MAX_PACKAGE_BYTES:
                    problems.append("skill packages exceed aggregate byte budget")
                    return sorted(set(problems))
                if metadata.st_size > MAX_FILE_BYTES:
                    problems.append("skill asset exceeds per-file byte budget: " + relative)
                    continue
                if stat.S_ISREG(metadata.st_mode) is False or metadata.st_mode & 0o111:
                    problems.append("executable or irregular skill asset: " + relative)
                    continue
                ext = Path(entry.name).suffix.lower()
                if ext not in DATA_EXTENSIONS:
                    problems.append("unreviewed executable/asset type: " + relative)
                    continue
                data = Path(entry.path).read_bytes()
                if ext in {".png", ".jpg", ".jpeg", ".webp"}:
                    if not _is_valid_image(ext, data):
                        problems.append("image asset type/header mismatch: " + relative)
                    continue
                if metadata.st_size > (MAX_MARKDOWN_BYTES if ext == ".md" else MAX_TEXT_BYTES):
                    problems.append("skill text exceeds bounded file size: " + relative)
                    continue
                try:
                    source = data.decode("utf-8", errors="strict")
                except UnicodeDecodeError:
                    problems.append("skill data is not valid UTF-8: " + relative)
                    continue
                if "\x00" in source or any(ord(ch) < 9 for ch in source):
                    problems.append("control bytes in skill data: " + relative)
                    continue
                if ext == ".json":
                    try:
                        json.loads(source)
                    except ValueError:
                        problems.append("invalid JSON skill data: " + relative)
                        continue
                for label, pattern in SUSPICIOUS:
                    if pattern.search(source):
                        problems.append(label + ": " + relative)
                if Path(entry.path) == directory / "SKILL.md":
                    seen_skill = True
                    problems.extend(identity + ": " + issue for issue in
                                    _validate_markdown(Path(entry.path), source, identity, item["kind"]))
                elif entry.name == "SKILL.md":
                    problems.append("nested SKILL.md would create competing skill authority: " + relative)
        if not seen_skill:
            problems.append(identity + ": root SKILL.md is missing")
    return sorted(set(problems))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", required=True)
    args = parser.parse_args()
    errors = admit_skills()
    print(json.dumps({"passed": not errors, "errors": errors,
                      "proof": "STATIC_SKILL_PACKAGE_ADMISSION_ONLY"}, indent=2))
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
