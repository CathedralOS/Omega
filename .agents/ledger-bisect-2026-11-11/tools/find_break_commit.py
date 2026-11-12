#!/usr/bin/env python3
"""Find nearest good to PIN then latest break commit (i-1) on first-parent chain."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

from find_good_exp import is_ledger_only, test_commit


def code_at(
    root: Path,
    tools: Path,
    witness: str,
    commits: list[str],
    i: int,
    cache: dict[str, int],
) -> int:
    c = commits[i]
    if is_ledger_only(root, c):
        return 125
    return test_commit(root, tools, witness, c, cache)


def main() -> int:
    root = Path(sys.argv[1])
    pin = sys.argv[2]
    witness = sys.argv[3]
    tools = Path(sys.argv[4])
    commits = subprocess.check_output(
        ["git", "rev-list", "--first-parent", pin], cwd=root, text=True
    ).splitlines()
    n = len(commits)
    cache: dict[str, int] = {}
    if code_at(root, tools, witness, commits, 0, cache) != 1:
        print("PIN not bad", file=sys.stderr)
        return 1
    lo, hi = 0, n - 1
    nearest_good = None
    while lo <= hi:
        mid = (lo + hi) // 2
        code = code_at(root, tools, witness, commits, mid, cache)
        if code == 125:
            # Treat ambiguous as bad for search purposes
            lo = mid + 1
            continue
        if code == 0:
            nearest_good = mid
            hi = mid - 1
        else:
            lo = mid + 1
    if nearest_good is None or nearest_good == 0:
        print("GOOD_NOT_FOUND or PIN adjacent good", file=sys.stderr)
        return 1
    bad_i = nearest_good - 1
    while bad_i >= 0:
        c = code_at(root, tools, witness, commits, bad_i, cache)
        if c == 1:
            break
        bad_i -= 1
    if bad_i < 0:
        print("BAD_BOUNDARY_NOT_FOUND", file=sys.stderr)
        return 1
    bad = commits[bad_i]
    good = commits[nearest_good]
    subject = subprocess.check_output(
        ["git", "log", "-1", "--format=%s", bad], cwd=root, text=True
    ).strip()
    print(
        f"BREAK {bad} bad_index={bad_i} good_index={nearest_good} good={good} builds={len(cache)} {subject}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
