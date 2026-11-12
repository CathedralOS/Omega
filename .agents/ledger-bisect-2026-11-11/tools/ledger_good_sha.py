#!/usr/bin/env python3
"""Resolve witness ledger @stamp to a full commit (first-parent reachable)."""
from __future__ import annotations

import subprocess
import sys


def stamp(fixture: str) -> str | None:
    for line in open("tests/omega/corpus_outcomes.txt"):
        if line.startswith(fixture + " "):
            for tok in line.split():
                if tok.startswith("@"):
                    return tok[1:]
    return None


def main() -> int:
    fix = sys.argv[1]
    st = stamp(fix)
    if not st:
        print("", end="")
        return 1
    full = subprocess.check_output(
        ["git", "rev-parse", "--verify", f"{st}^{{commit}}"],
        text=True,
    ).strip()
    print(full)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
