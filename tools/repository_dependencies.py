"""Derived, bounded source-import graph for the existing affected-proof planner.

Edges point consumer -> dependency. This parser covers explicit relative
TypeScript/Svelte imports, Rust module/use paths, and local Python AST imports.
It is not a compiler, language server, semantic truth store, or full call graph.
Graph state is reconstructed from current source and is never persisted.
"""
from __future__ import annotations

import ast
from collections import defaultdict, deque
from pathlib import Path
import posixpath
import re

FRONT = "EngineData/Frontend/RustApp/"
WORKER = "EngineData/Backend/LocalWorker/WorkerRuntime/"
RUST = FRONT + "src-tauri/src/"
SOURCE_DIRS = (
    (FRONT + "src/", (".ts", ".svelte")),
    (FRONT + "scripts/tests/", (".ts",)),
    (RUST, (".rs",)),
    (WORKER, (".py",)),
)
MAX_FILES = 360
MAX_BYTES = 4_000_000
MAX_SOURCE_BYTES = 100_000
MAX_AFFECTED = 360
JS_IMPORT = re.compile(
    r"""(?:\bfrom\s*|\bimport\s*|\bimport\s*\(\s*|\brequire\s*\(\s*)["'](\.[^"']+)["']"""
)
RUST_MOD = re.compile(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([a-z_][a-z_0-9]*)\s*;")
RUST_USE = re.compile(r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?use\s+([\s\S]*?);")
RUST_CRATE_REF = re.compile(r"\bcrate::([a-z_][a-z_0-9]*(?:::[a-z_][a-z_0-9]*)+)")


def _scope(path: str) -> str:
    if path.startswith(FRONT + "src/") or path.startswith(FRONT + "scripts/tests/"):
        return "typescript"
    if path.startswith(RUST):
        return "rust"
    if path.startswith(WORKER):
        return "python"
    return "outside"


def _sources(root: Path) -> dict[str, str]:
    files: list[tuple[str, Path]] = []
    for directory, endings in SOURCE_DIRS:
        base = root / directory
        if not base.is_dir() or base.is_symlink():
            raise ValueError(f"source graph root is unavailable or linked: {directory}")
        for path in sorted(base.rglob("*")):
            if path.suffix not in endings or not path.is_file():
                continue
            if path.is_symlink():
                raise ValueError("source graph contains a symlinked source")
            files.append((path.relative_to(root).as_posix(), path))
            if len(files) > MAX_FILES:
                raise ValueError("source graph exceeds bounded file count")
    if sum(path.stat().st_size for _, path in files) > MAX_BYTES:
        raise ValueError("source graph exceeds bounded source-byte budget")
    results = {}
    for relative, path in files:
        if path.stat().st_size > MAX_SOURCE_BYTES:
            raise ValueError(f"source graph owner exceeds per-file budget: {relative}")
        results[relative] = path.read_text(encoding="utf-8")
    return results


def _js_edges(path: str, body: str, known: set[str]) -> tuple[set[str], list[str]]:
    dependencies: set[str] = set()
    missing: list[str] = []
    for match in JS_IMPORT.finditer(body):
        raw = match.group(1)
        if "$" in raw or "\n" in raw:
            missing.append(raw[:120])
            continue
        stem = posixpath.normpath(posixpath.join(posixpath.dirname(path), raw))
        candidates = [
            stem, stem + ".ts", stem + ".svelte",
            stem + "/index.ts", stem + "/index.svelte",
        ]
        target = next((candidate for candidate in candidates if candidate in known), None)
        if target:
            dependencies.add(target)
        else:
            missing.append(raw[:120])
    return dependencies, missing


def _rust_module_paths(known: set[str]) -> dict[str, str]:
    modules: dict[str, str] = {}
    for path in sorted(known):
        if not path.startswith(RUST) or not path.endswith(".rs"):
            continue
        suffix = path[len(RUST):]
        parts = suffix[:-3].split("/")
        if parts[-1] == "mod":
            parts.pop()
        elif parts[-1] in {"lib", "main"} and len(parts) == 1:
            parts.pop()
        if parts:
            module_name = "::".join(parts)
            if module_name in modules:
                raise ValueError(f"duplicate Rust module identity: {module_name}")
            modules[module_name] = path
    return modules


def _rust_mod_dir(path: str) -> str:
    directory = posixpath.dirname(path)
    name = posixpath.basename(path)
    if name not in {"main.rs", "lib.rs", "mod.rs"}:
        directory = posixpath.join(directory, name[:-3])
    return directory


def _rust_edges(path: str, body: str, known: set[str],
                modules: dict[str, str]) -> tuple[set[str], list[str]]:
    dependencies: set[str] = set()
    missing: list[str] = []
    for match in RUST_MOD.finditer(body):
        name = match.group(1)
        base = posixpath.join(_rust_mod_dir(path), name)
        destination = next((candidate for candidate in (base + ".rs", base + "/mod.rs")
                            if candidate in known), None)
        if destination:
            dependencies.add(destination)
        else:
            missing.append("mod " + name)
    origin = path[len(RUST):-3].split("/")
    if origin[-1] == "mod":
        origin.pop()
    elif origin[-1] in {"main", "lib"} and len(origin) == 1:
        origin.pop()
    for match in RUST_USE.finditer(body):
        expression = match.group(1).strip()
        raw = expression.split("{", 1)[0].rstrip(":").strip()
        if not raw.startswith(("crate::", "super::", "self::")):
            continue
        parts = [value.strip() for value in raw.split("::") if value.strip()]
        if not parts:
            continue
        if parts[0] == "crate":
            names = parts[1:]
        else:
            names = origin.copy()
            while parts and parts[0] in {"super", "self"}:
                if parts.pop(0) == "super":
                    if names:
                        names.pop()
                    else:
                        missing.append(expression[:120])
            names.extend(parts)
        for width in range(len(names), 0, -1):
            target = modules.get("::".join(names[:width]))
            if target:
                if target != path:
                    dependencies.add(target)
                break
        # A use ending in a function/type is resolved by its nearest file.
        # Unknown symbols and external macros are outside this partial graph.
    for match in RUST_CRATE_REF.finditer(body):
        pieces = match.group(1).split("::")
        for width in range(len(pieces), 0, -1):
            target = modules.get("::".join(pieces[:width]))
            if target:
                if target != path:
                    dependencies.add(target)
                break
    return dependencies, missing


def _python_modules(known: set[str]) -> dict[str, str]:
    modules = {}
    for path in sorted(known):
        if not path.startswith(WORKER) or not path.endswith(".py"):
            continue
        parts = path[len(WORKER):-3].split("/")
        if parts[-1] == "__init__":
            parts.pop()
        name = ".".join(parts)
        if name:
            if name in modules:
                raise ValueError(f"duplicate Python module identity: {name}")
            modules[name] = path
    return modules


def _python_edges(path: str, body: str, modules: dict[str, str]) -> set[str]:
    dependencies: set[str] = set()
    try:
        tree = ast.parse(body, filename=path)
    except SyntaxError as exc:
        raise ValueError(f"Python source parser cannot establish graph: {path}: {exc}") from exc
    parts = path[len(WORKER):-3].split("/")
    package = parts if parts[-1] == "__init__" else parts[:-1]
    if package and package[-1] == "__init__":
        package = package[:-1]
    for node in ast.walk(tree):
        names: list[str] = []
        if isinstance(node, ast.Import):
            names = [item.name for item in node.names]
        elif isinstance(node, ast.ImportFrom):
            prefix = package[:]
            if node.level:
                remove = node.level - 1
                prefix = prefix[:len(prefix)-remove] if remove <= len(prefix) else []
            else:
                prefix = []
            prefix.extend((node.module or "").split(".") if node.module else [])
            name = ".".join(prefix)
            names = [name] if name else []
            names += [name + "." + alias.name for alias in node.names] if name else [
                ".".join(prefix + [alias.name]) for alias in node.names
            ]
        for name in names:
            if not name:
                continue
            elements = name.split(".")
            for width in range(len(elements), 0, -1):
                target = modules.get(".".join(elements[:width]))
                if target:
                    if target != path:
                        dependencies.add(target)
                    break
    return dependencies


def build_source_graph(root: Path) -> dict:
    """Bounded source read, no files written. Graph cannot prove full semantics."""
    sources = _sources(root)
    known = set(sources)
    rust_modules = _rust_module_paths(known)
    py_modules = _python_modules(known)
    reverse: dict[str, set[str]] = defaultdict(set)
    edges: set[tuple[str, str, str]] = set()
    unresolved: list[dict[str, str]] = []
    for path, body in sources.items():
        scope = _scope(path)
        if scope == "typescript":
            deps, missing = _js_edges(path, body, known)
        elif scope == "rust":
            deps, missing = _rust_edges(path, body, known, rust_modules)
        else:
            deps, missing = _python_edges(path, body, py_modules), []
        for dependency in deps:
            if dependency not in known or dependency == path:
                continue
            edges.add((path, dependency, scope))
            reverse[dependency].add(path)
        for raw in missing:
            unresolved.append({"importer": path, "specifier": raw, "kind": scope})
    return {
        "reverse": reverse,
        "edges": sorted(edges),
        "indexed": set(sources),
        "unresolved": sorted(unresolved, key=lambda entry: (entry["importer"], entry["specifier"])),
        "indexedByLanguage": {
            name: sum(_scope(path) == name for path in sources)
            for name in ("typescript", "rust", "python")
        },
        "proof": "PARTIAL_SOURCE_IMPORTS_NOT_COMPILE_PROOF",
    }


def affected_consumers(graph: dict, changed: str) -> list[str]:
    """Transitive reverse imports; cyclic source modules do not cause recursion."""
    reached: set[str] = set()
    pending: deque[str] = deque([changed])
    while pending:
        dependency = pending.popleft()
        for consumer in sorted(graph["reverse"].get(dependency, ())):
            if consumer == changed or consumer in reached:
                continue
            reached.add(consumer)
            if len(reached) > MAX_AFFECTED:
                raise ValueError("affected source expansion exceeded bounded budget")
            pending.append(consumer)
    return sorted(reached)


def relevant_unknown_imports(graph: dict, paths: set[str]) -> list[dict[str, str]]:
    """Only unknown imports on the affected projection require immediate review."""
    return [item for item in graph["unresolved"] if item["importer"] in paths]
