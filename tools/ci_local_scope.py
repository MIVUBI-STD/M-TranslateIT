"""Select narrow, honest Local-push CI checks from the exact Git change range.

This is source-only routing; selected checks do not prove unselected compiler,
Python model, Windows device, installer, or release behavior.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import subprocess

SHA = re.compile(r"[0-9a-f]{40}\Z")
ROOT = Path(__file__).resolve().parents[1]
FRONT = "EngineData/Frontend/RustApp/"
RUST = FRONT + "src-tauri/"
WORKER = "EngineData/Backend/LocalWorker/WorkerRuntime/"


def route(paths: list[str]) -> dict[str, bool]:
    flags = {"repository": False, "frontend": False, "rust": False,
             "worker": False, "review": False}
    for path in paths:
        if path.startswith(RUST):
            flags["rust"] = True
        elif path.startswith(FRONT):
            flags["frontend"] = True
        elif path.startswith(WORKER):
            flags["worker"] = True
        elif path.startswith("EngineData/Backend/RuntimeAssets/"):
            flags["worker"] = True
            flags["review"] = True
        elif path.startswith((".github/", "tools/", "tooling/", ".agents/",
                              "docs/", "planning/")) or path in {
            "AGENTS.md", "GITHUB_RULES.md", "README.md", "CONTEXT.md",
            "CONTRIBUTING.md", "SECURITY.md", "manage.cmd",
            ".gitignore", ".gitattributes", ".editorconfig",
        }:
            flags["repository"] = True
        else:
            # Unknown source ownership requires additional review. Do not silently
            # treat an unmatched file as having no relevant proof requirement.
            flags["repository"] = True
            flags["review"] = True
    return flags


def git(*args: str) -> bytes:
    return subprocess.check_output(["git", *args], cwd=ROOT)


def changed_paths(before: str | None, head: str) -> tuple[list[str], str]:
    if not SHA.fullmatch(head) or git("rev-parse", "HEAD").decode().strip() != head:
        raise ValueError("checkout must be the exact requested SHA")
    if before and SHA.fullmatch(before) and before != "0" * 40:
        try:
            git("cat-file", "-e", before + "^{commit}")
            raw = git("diff", "--name-only", "-z", "--no-renames", before, head)
            return [p.decode("utf-8") for p in raw.split(b"\0") if p], "exact push range"
        except subprocess.CalledProcessError:
            pass
    # Manual targeted dispatch has no GitHub push.before. Scope it to the
    # explicitly selected HEAD commit, not the entire repository; never assume
    # this for a push with missing ancestry.
    if os.environ.get("GITHUB_EVENT_NAME") == "workflow_dispatch" and not before:
        try:
            parent = git("rev-parse", "HEAD^").decode("ascii").strip()
            raw = git("diff", "--name-only", "-z", "--no-renames", parent, head)
            selected = [p.decode("utf-8") for p in raw.split(b"\\0") if p]
            if selected:
                return selected, "manual selected commit"
        except subprocess.CalledProcessError:
            pass
    # Unknown push ancestry or a root-commit dispatch must not masquerade
    # as a docs-only or empty diff. Widen to tracked files and report it.
    raw = git("ls-files", "-z")
    return [p.decode("utf-8") for p in raw.split(b"\0") if p], "all tracked fallback"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--head", required=True)
    parser.add_argument("--before")
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    if os.environ.get("GITHUB_REF") != "refs/heads/Local":
        parser.error("only Local may use automatic selected source CI")
    paths, coverage = changed_paths(args.before, args.head)
    selected = route(paths)
    if not paths:
        parser.error("empty changed scope is not proof")
    output = Path(args.output)
    with output.open("a", encoding="utf-8") as stream:
        for key, selected_flag in selected.items():
            stream.write(f"{key}={str(selected_flag).lower()}\n")
    print(f"SHA={args.head}; coverage={coverage}; paths={len(paths)}; domains={selected}")
    print("Unselected Rust compiler, worker inference, and TARGET_WINDOWS proof remain NOT EXECUTED.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
