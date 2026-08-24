#!/usr/bin/env python3
"""Verify deterministic product-compiler source checkpoint manifests."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path, PurePosixPath


ROOT = Path(__file__).resolve().parents[2]
CHECKPOINT_DIR = Path(__file__).resolve().parent
SCHEMA = "omega.product-compiler-source-checkpoint.v1"
BUILD_PRELUDE_OWNER = (
    "bootstrap/onramps/omega-rust/omega/orchestration/omega-compiler/"
    "src/pipeline/stages.rs"
)
BUILD_PRELUDE_SNAPSHOT = "compiler/source-checkpoints/inputs/build-prelude.omg"


def fail(message: str) -> None:
    raise SystemExit(f"product source checkpoint: {message}")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate_relative_path(spelling: str) -> None:
    path = PurePosixPath(spelling)
    if path.is_absolute() or ".." in path.parts or str(path) != spelling:
        fail(f"non-canonical repository path {spelling!r}")


def verify_build_prelude_snapshot() -> None:
    owner = (ROOT / BUILD_PRELUDE_OWNER).read_text(encoding="utf-8")
    marker = 'const BUILD_PRELUDE: &str = r#"'
    start = owner.find(marker)
    if start < 0:
        fail("cannot locate BUILD_PRELUDE in its pinned owner")
    start += len(marker)
    end = owner.find('"#;', start)
    if end < 0:
        fail("cannot locate the end of BUILD_PRELUDE in its pinned owner")
    injected = owner[start:end].encode("utf-8")
    snapshot = (ROOT / BUILD_PRELUDE_SNAPSHOT).read_bytes()
    if snapshot != injected:
        fail("build-prelude snapshot differs from the compiler-injected source")


def verify(manifest_path: Path) -> None:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("schema") != SCHEMA:
        fail(f"{manifest_path.name}: unsupported schema")

    groups = ("compiled_sources", "virtual_sources", "generated_inputs")
    entries: list[tuple[str, str, bytes]] = []
    known_paths: set[str] = set()
    for group in groups:
        rows = manifest.get(group)
        if not isinstance(rows, list):
            fail(f"{manifest_path.name}: {group} must be an array")
        paths = [row.get("path") for row in rows]
        if paths != sorted(paths) or len(paths) != len(set(paths)):
            fail(f"{manifest_path.name}: {group} paths must be unique and sorted")
        for row in rows:
            spelling = row.get("path")
            expected = row.get("sha256")
            if not isinstance(spelling, str) or not isinstance(expected, str):
                fail(f"{manifest_path.name}: malformed {group} entry")
            validate_relative_path(spelling)
            path = ROOT / spelling
            if not path.is_file():
                fail(f"{manifest_path.name}: missing {spelling}")
            content = path.read_bytes()
            actual = hashlib.sha256(content).hexdigest()
            if actual != expected:
                fail(f"{manifest_path.name}: digest mismatch for {spelling}")
            entries.append((group, spelling, content))
            known_paths.add(spelling)

    compiled_count = len(manifest["compiled_sources"]) + len(manifest["virtual_sources"])
    if manifest.get("source_file_count") != compiled_count:
        fail(f"{manifest_path.name}: source_file_count does not match closure")

    edges = manifest.get("dependency_edges")
    if not isinstance(edges, list):
        fail(f"{manifest_path.name}: dependency_edges must be an array")
    edge_keys = [(row.get("from"), row.get("kind"), row.get("to")) for row in edges]
    if edge_keys != sorted(edge_keys) or len(edge_keys) != len(set(edge_keys)):
        fail(f"{manifest_path.name}: dependency edges must be unique and sorted")
    for source, kind, target in edge_keys:
        if not all(isinstance(value, str) for value in (source, kind, target)):
            fail(f"{manifest_path.name}: malformed dependency edge")
        if source not in known_paths or target not in known_paths:
            fail(f"{manifest_path.name}: dependency edge has unknown endpoint")

    aggregate = hashlib.sha256()
    aggregate.update(b"omega.product-compiler-source-checkpoint.v1\0")
    for group, spelling, content in entries:
        aggregate.update(group.encode("utf-8") + b"\0")
        aggregate.update(spelling.encode("utf-8") + b"\0")
        aggregate.update(len(content).to_bytes(8, "little"))
        aggregate.update(content)
    if aggregate.hexdigest() != manifest.get("aggregate_sha256"):
        fail(f"{manifest_path.name}: aggregate digest mismatch")


def main() -> None:
    manifests = sorted(CHECKPOINT_DIR.glob("checkpoint-*.json"))
    if not manifests:
        fail("no checkpoint manifests")
    for manifest in manifests:
        verify(manifest)
    verify_build_prelude_snapshot()
    print(f"verified {len(manifests)} product source checkpoint manifest(s)")


if __name__ == "__main__":
    main()
