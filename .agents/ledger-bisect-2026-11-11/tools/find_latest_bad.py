#!/usr/bin/env python3
"""Find latest first-parent break: min i>0 with good at i and bad at i-1 (newer)."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

from find_good_exp import is_ledger_only, test_commit


def main() -> int:
    root = Path(sys.argv[1])
    pin = sys.argv[2]
    witness = sys.argv[3]
    tools = Path(sys.argv[4])
    max_scan = int(os.environ.get("LATEST_BAD_MAX", "120"))
    commits = subprocess.check_output(
        ["git", "rev-list", "--first-parent", pin], cwd=root, text=True
    ).splitlines()
    cache: dict[str, int] = {}
    if test_commit(root, tools, witness, commits[0], cache) != 1:
        print("PIN not bad", file=sys.stderr)
        return 1
    limit = min(len(commits), max_scan)
    codes = []
    for i in range(limit):
        c = commits[i]
        if is_ledger_only(root, c):
            codes.append(125)
            continue
        codes.append(test_commit(root, tools, witness, c, cache))
    for i in range(1, limit):
        if codes[i] == 0 and codes[i - 1] == 1:
            bad = commits[i - 1]
            subject = subprocess.check_output(
                ["git", "log", "-1", "--format=%s", bad], cwd=root, text=True
            ).strip()
            print(
                f"LATEST_BAD {bad} bad_index={i-1} good_index={i} builds={len(cache)} {subject}"
            )
            return 0
    print("LATEST_BAD_NOT_FOUND in scan window", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
