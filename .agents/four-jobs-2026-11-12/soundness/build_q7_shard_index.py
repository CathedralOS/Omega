#!/usr/bin/env python3
"""Build Q7 hit index for shard fixture paths (PR #2638 scan)."""
from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent / "q7_shard_index.json"
SCAN = Path(__file__).resolve().parent / "q7_scan_ref.py"


def fixture_list() -> list[str]:
    proc = subprocess.run(
        [
            "git",
            "ls-files",
            "tests/omega/pass/**/main.omg",
            "tests/omega/run/**/main.omg",
        ],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=True,
    )
    lines = sorted(proc.stdout.splitlines())
    return [p for i, p in enumerate(lines) if i % 8 == 2]


def main() -> None:
    spec = importlib.util.spec_from_file_location("q7_scan_ref", SCAN)
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    sys.modules["q7_scan_ref"] = mod
    spec.loader.exec_module(mod)
    index: dict[str, list[dict]] = {}
    for rel in fixture_list():
        path = REPO / rel
        hits = mod.scan_file(path)
        if hits:
            index[rel] = [
                {
                    "binding": h.binding,
                    "line": h.line,
                    "kind": h.kind,
                    "snippet": h.snippet,
                }
                for h in hits
                if not h.review
            ]
    OUT.write_text(json.dumps(index, indent=2))
    print("fixtures_with_q7", len(index), "hits", sum(len(v) for v in index.values()))


if __name__ == "__main__":
    main()
