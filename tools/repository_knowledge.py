"""Read-only, source-derived document Catalog → Graph → bounded Retrieval.

Frontmatter owns stable document IDs. Real Markdown links own edges. This module
never persists a second knowledge store or promotes retrieval ranking into proof.
"""
from __future__ import annotations

import argparse
from collections import Counter, deque
import json
import posixpath
import re
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
META = re.compile(r"\A---\r?\n(.*?)\r?\n---\r?\n", re.DOTALL)
LINK = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
HEAD = re.compile(r"(?m)^#\s+(.+)$")
SECTION = re.compile(r"(?m)^(#{2,3})\s+(.+)$")
WORDS = re.compile(r"[a-z0-9]+", re.I)
FIELDS = {"id", "class", "domain", "role", "authority", "lifecycle"}
ROLES = {"ROUTER", "WORKFLOW", "CONTRACT", "DOMAIN", "ARCHITECTURE", "GUIDE", "REFERENCE"}
AUTHORITIES = {"CANONICAL", "REFERENCE", "HISTORICAL", "DERIVED"}
ROUTERS = (
    "docs/README.md",
    "docs/foundation/README.md",
    "docs/knowledge/README.md",
    "docs/system/README.md",
    "docs/knowledge/decisions/README.md",
    "docs/knowledge/operations/README.md",
    "docs/knowledge/skills/README.md",
)


def read_metadata(content: str, path: str) -> dict[str, str]:
    match = META.match(content)
    if not match:
        raise ValueError(f"{path}: missing document metadata")
    fields: dict[str, str] = {}
    for line in match.group(1).splitlines():
        if ":" not in line:
            raise ValueError(f"{path}: malformed metadata line")
        key, value = line.split(":", 1)
        if key in fields or key not in FIELDS or not value.strip():
            raise ValueError(f"{path}: duplicate/unknown/empty metadata {key!r}")
        fields[key] = value.strip()
    if set(fields) != FIELDS:
        raise ValueError(f"{path}: metadata must contain exactly {sorted(FIELDS)}")
    if not re.fullmatch(r"document\.[a-z0-9-]+(?:\.[a-z0-9-]+)*", fields["id"]):
        raise ValueError(f"{path}: invalid stable document ID")
    if fields["class"] != "DOCUMENT" or fields["role"] not in ROLES:
        raise ValueError(f"{path}: invalid document classification")
    if fields["domain"] not in {"docs", "knowledge", "foundation", "system"}:
        raise ValueError(f"{path}: unsupported document domain")
    if fields["authority"] not in AUTHORITIES or fields["lifecycle"] not in {"ACTIVE", "RETIRED"}:
        raise ValueError(f"{path}: invalid authority/lifecycle")
    return fields


def document_target(source: str, link: str) -> str | None:
    raw = unquote(link.strip().strip("<>")).split("#", 1)[0].split("?", 1)[0]
    if not raw or "://" in raw or raw.startswith(("mailto:", "/", "#")) or "\\" in raw:
        return None
    if not raw.lower().endswith(".md"):
        return None
    dest = posixpath.normpath(posixpath.join(posixpath.dirname(source), raw))
    return dest if dest.startswith("docs/") else None


def catalog(root: Path = ROOT) -> tuple[list[dict], list[dict], list[str]]:
    documents: list[dict] = []
    contents: dict[str, str] = {}
    issues: list[str] = []
    if not (root / "docs").is_dir():
        return [], [], ["missing docs/ root"]
    identities: set[str] = set()
    for file in sorted((root / "docs").rglob("*.md")):
        path = file.relative_to(root).as_posix()
        content = file.read_text(encoding="utf-8")
        try:
            meta = read_metadata(content, path)
        except ValueError as exc:
            issues.append(str(exc))
            continue
        if meta["id"] in identities:
            issues.append(f"duplicate document ID: {meta['id']}")
        identities.add(meta["id"])
        actual_domain = "docs" if path == "docs/README.md" else path.split("/")[1]
        if meta["domain"] != actual_domain:
            issues.append(f"{path}: metadata domain disagrees with directory")
        title = HEAD.search(content)
        documents.append({**meta, "path": path, "title": title.group(1).strip() if title else path})
        contents[path] = content
    by_path = {doc["path"]: doc for doc in documents}
    edges: list[dict] = []
    seen_edges: set[tuple[str, str]] = set()
    for path, content in contents.items():
        for match in LINK.finditer(content):
            target = document_target(path, match.group(1))
            if target is None:
                continue
            if target not in by_path:
                issues.append(f"{path}: broken document link {target}")
                continue
            edge = by_path[path]["id"], by_path[target]["id"]
            if edge in seen_edges:
                continue
            seen_edges.add(edge)
            relation = "ROUTES_TO" if by_path[path]["role"] == "ROUTER" else "RELATES_TO"
            edges.append({"from": edge[0], "to": edge[1], "type": relation})
    edges.sort(key=lambda edge: (edge["from"], edge["to"], edge["type"]))
    return documents, edges, issues


def verify_knowledge(root: Path = ROOT) -> list[str]:
    documents, edges, issues = catalog(root)
    by_path = {doc["path"]: doc for doc in documents}
    for path in ROUTERS:
        if path not in by_path:
            issues.append(f"missing documentation router: {path}")
        elif by_path[path]["role"] != "ROUTER":
            issues.append(f"invalid router role: {path}")
    if "docs/README.md" not in by_path:
        return sorted(set(issues))
    adjacency: dict[str, list[str]] = {}
    for edge in edges:
        adjacency.setdefault(edge["from"], []).append(edge["to"])
    seen: set[str] = set()
    queue = deque([by_path["docs/README.md"]["id"]])
    while queue:
        node = queue.popleft()
        if node in seen:
            continue
        seen.add(node)
        queue.extend(adjacency.get(node, []))
    for doc in documents:
        if doc["lifecycle"] == "ACTIVE" and doc["id"] not in seen:
            issues.append(f"document not reachable from canonical router: {doc['path']}")
    return sorted(set(issues))


def sections(text: str) -> list[tuple[str, str, int]]:
    headers = list(SECTION.finditer(text))
    if not headers:
        return [("(document)", text, 1)]
    result = [("(intro)", text[:headers[0].start()], 1)]
    for index, header in enumerate(headers):
        stop = headers[index + 1].start() if index + 1 < len(headers) else len(text)
        result.append((header.group(2).strip(), text[header.start():stop], text.count("\n", 0, header.start()) + 1))
    return result


def retrieve(query: str, domain: str, root: Path = ROOT, limit: int = 8) -> list[dict]:
    if domain not in {"foundation", "knowledge", "system", "all"}:
        raise ValueError("retrieve only from a selected domain unless explicitly all")
    if limit < 1 or limit > 25:
        raise ValueError("retrieval limit must be 1..25")
    terms = WORDS.findall(query.lower())
    if not terms:
        raise ValueError("empty retrieval query")
    docs, _, errors = catalog(root)
    if errors:
        raise ValueError("invalid documentation catalog: " + "; ".join(errors[:3]))
    results = []
    for doc in docs:
        if domain != "all" and doc["domain"] != domain:
            continue
        if doc["lifecycle"] != "ACTIVE":
            continue
        content = (root / doc["path"]).read_text(encoding="utf-8")
        for heading, body, line in sections(content):
            haystack = (doc["title"] + " " + heading + " " + body).lower()
            if not all(term in haystack for term in terms):
                continue
            score = sum(6 * int(term in doc["title"].lower()) + 4 * int(term in heading.lower()) + min(haystack.count(term), 5) for term in terms)
            results.append({
                "id": doc["id"], "path": doc["path"], "authority": doc["authority"],
                "title": doc["title"], "section": heading, "line": line,
                "score": score, "excerpt": " ".join(body.split())[:240],
            })
    results.sort(key=lambda item: (-item["score"], item["path"], item["line"]))
    return results[:limit]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    opts = parser.add_mutually_exclusive_group(required=True)
    opts.add_argument("--check", action="store_true")
    opts.add_argument("--summary", action="store_true")
    opts.add_argument("--id")
    opts.add_argument("--query")
    parser.add_argument("--domain", choices=["foundation", "knowledge", "system"])
    parser.add_argument("--all", action="store_true")
    parser.add_argument("--limit", type=int, default=8)
    args = parser.parse_args()
    if args.check:
        issues = verify_knowledge()
        print(json.dumps({"passed": not issues, "issues": issues}, indent=2))
        return int(bool(issues))
    docs, edges, issues = catalog()
    if issues:
        print(json.dumps({"passed": False, "issues": issues}, indent=2))
        return 1
    if args.summary:
        print(json.dumps({"documents": len(docs), "relationships": len(edges), "byDomain": dict(Counter(d["domain"] for d in docs)), "byAuthority": dict(Counter(d["authority"] for d in docs))}, indent=2))
    elif args.id:
        result = next((item for item in docs if item["id"] == args.id), None)
        if result is None:
            parser.error("unknown stable document ID")
        print(json.dumps({**result, "sections": [{"heading": h, "line": n} for h, _, n in sections((ROOT / result["path"]).read_text(encoding="utf-8"))]}, indent=2))
    else:
        if not args.all and not args.domain:
            parser.error("specify --domain to bound retrieval; use --all only for diagnostic broad search")
        try:
            results = retrieve(args.query, "all" if args.all else args.domain, limit=args.limit)
        except ValueError as exc:
            parser.error(str(exc))
        print(json.dumps({"results": results}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
