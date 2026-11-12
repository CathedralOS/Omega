#!/usr/bin/env python3
"""Walk first-parent history to find newest commit where witness passes bisect_test."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
TOOLS = Path(__file__).resolve().parent
WITNESS = sys.argv[1]
START_BAD = sys.argv[2] if len(sys.argv) > 2 else "741270df60f7"
MAX = int(sys.argv[3]) if len(sys.argv) > 3 else 80


def verify(commit: str) -> int:
    r = subprocess.run(
        ["bash", str(TOOLS / "verify_witness.sh"), commit, WITNESS, "1"],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    for line in r.stdout.splitlines():
        if line.startswith("run=1"):
            return int(line.split("rc=")[1].split()[0])
    return 125


def main() -> int:
    commits = subprocess.check_output(
        ["git", "rev-list", "--first-parent", START_BAD],
        cwd=REPO,
        text=True,
    ).splitlines()
    commits = commits[:MAX]
    for c in commits:
        rc = verify(c)
        print(c, "rc", rc, flush=True)
        if rc == 0:
            print("GOOD", c)
            return 0
    print("NO_GOOD")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
