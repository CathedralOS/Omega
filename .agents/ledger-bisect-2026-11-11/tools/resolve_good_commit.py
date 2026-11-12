#!/usr/bin/env python3
"""Resolve a bisect-good commit for a fixture (ledger stamp, then git -S fallback)."""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]


def stamp_from_ledger(fixture: str) -> str | None:
    for rel in (
        "tests/omega/corpus_outcomes.txt",
        "tests/omega/corpus_both_linux_arm64.txt",
    ):
        p = REPO / rel
        if not p.is_file():
            continue
        for line in p.read_text().splitlines():
            if not line.startswith(fixture + " "):
                continue
            for tok in line.split():
                if tok.startswith("@") and len(tok) > 1:
                    return tok[1:].split("+")[0]
    return None


def commit_exists(sha: str) -> bool:
    if not sha:
        return False
    # allow abbrev if unique
    r = subprocess.run(
        ["git", "rev-parse", "--verify", f"{sha}^{{commit}}"],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    return r.returncode == 0


def resolve_stamp(stamp: str) -> str | None:
    if commit_exists(stamp):
        return subprocess.check_output(
            ["git", "rev-parse", stamp], cwd=REPO, text=True
        ).strip()
    # match unique 12-char prefix across all commits
    r = subprocess.run(
        ["git", "rev-list", "--all"],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    hits = [h for h in r.stdout.splitlines() if h.startswith(stamp)]
    return hits[0] if len(hits) == 1 else None


def search_checked_in_history(fixture: str) -> str | None:
    needle = f"{fixture} checked"
    r = subprocess.run(
        [
            "git",
            "log",
            "--first-parent",
            "-1",
            "--format=%H",
            "-S",
            needle,
            "--",
            "tests/omega/corpus_outcomes.txt",
        ],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    sha = r.stdout.strip()
    if sha:
        return sha
    # introduction commit for fixture file
    parts = fixture.split("/")
    if len(parts) >= 3:
        p = REPO / "tests/omega" / parts[0] / parts[1] / parts[2]
        if p.is_dir():
            r2 = subprocess.run(
                [
                    "git",
                    "log",
                    "--first-parent",
                    "-1",
                    "--format=%H",
                    "--",
                    str(p.relative_to(REPO)),
                ],
                cwd=REPO,
                capture_output=True,
                text=True,
            )
            return r2.stdout.strip() or None
    return None


def main() -> int:
    fixture = sys.argv[1]
    stamp = stamp_from_ledger(fixture)
    if stamp:
        full = resolve_stamp(stamp)
        if full:
            print(full)
            return 0
    fallback = search_checked_in_history(fixture)
    if fallback:
        print(fallback)
        return 0
    print("", end="")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
