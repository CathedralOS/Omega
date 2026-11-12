#!/usr/bin/env python3
"""Resolve ledger @stamp or last corpus_outcomes row for bisect good commit."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
LEDGER = REPO / "tests/omega/corpus_outcomes.txt"


def main() -> int:
    fix = sys.argv[1]
    stamp: str | None = None
    for line in LEDGER.read_text().splitlines():
        if not line.startswith(fix + " "):
            continue
        for tok in line.split():
            if tok.startswith("@"):
                stamp = tok[1:]
                break
        break
    if not stamp:
        print("", end="")
        return 1
    # expand short stamp to object
    for length in (40, len(stamp), 12):
        cand = stamp[:length]
        r = subprocess.run(
            ["git", "-C", str(REPO), "rev-parse", "--verify", f"{cand}^{{commit}}"],
            capture_output=True,
            text=True,
        )
        if r.returncode == 0:
            print(r.stdout.strip())
            return 0
    r = subprocess.run(
        ["git", "-C", str(REPO), "rev-list", "--all"],
        capture_output=True,
        text=True,
    )
    for sha in r.stdout.splitlines():
        if sha.startswith(stamp):
            print(sha)
            return 0
    # Ledger-only worktree stamp: use newest corpus_outcomes introduction as good anchor.
    r = subprocess.run(
        [
            "git",
            "-C",
            str(REPO),
            "log",
            "-S",
            fix,
            "--first-parent",
            "-n",
            "1",
            "--format=%H",
            "tests/omega/corpus_outcomes.txt",
        ],
        capture_output=True,
        text=True,
    )
    if r.returncode == 0 and r.stdout.strip():
        print(r.stdout.strip())
        return 0
    print(stamp, file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
