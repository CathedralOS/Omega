#!/usr/bin/env python3
"""Binary search first-parent chain for earliest index (closest to PIN) that is bad."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

from find_good_exp import is_ledger_only, test_commit


def main() -> int:
    root = Path(sys.argv[1])
    pin = sys.argv[2]
    good = sys.argv[3]
    witness = sys.argv[4]
    tools = Path(sys.argv[5])
    commits = subprocess.check_output(
        ["git", "rev-list", "--first-parent", pin], cwd=root, text=True
    ).splitlines()
    gi = commits.index(good)
    cache: dict[str, int] = {}
    lo, hi = 0, gi
    while hi - lo > 1:
        mid = (lo + hi) // 2
        c = commits[mid]
        if is_ledger_only(root, c):
            if mid > lo:
                hi = mid
            else:
                lo = mid
            continue
        code = test_commit(root, tools, witness, c, cache)
        if code == 1:
            lo = mid
        elif code == 0:
            hi = mid
        else:
            lo = mid
    # lo should be bad, hi should be good (neighbor pair)
    bad = commits[lo]
    subject = subprocess.check_output(
        ["git", "log", "-1", "--format=%s", bad], cwd=root, text=True
    ).strip()
    m = subprocess.check_output(
        ["git", "log", "-1", "--format=%s", bad], cwd=root, text=True
    )
    print(f"FIRST_BAD {bad} index={lo} good_neighbor={commits[hi]} builds={len(cache)} {subject}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
