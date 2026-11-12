#!/usr/bin/env python3
"""Newest first-parent ancestor of PIN where witness reaches good_status."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PIN = sys.argv[2] if len(sys.argv) > 2 else "51e8c45c793007f211d0d9baee7de16767172444"
WITNESS = sys.argv[1]
SCRIPT = Path("/tmp/ledger-bisect-pin/tools/bisect_witness.sh")
JUMPS = [0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384]


def run_test() -> int:
    env = {**dict(**__import__("os").environ), "BISECT_WITNESS": WITNESS}
    return subprocess.call(["bash", str(SCRIPT)], cwd=ROOT, env=env)


def probe(revs: list[str], idx: int) -> int:
    subprocess.check_call(["git", "-C", str(ROOT), "checkout", "-q", revs[idx]])
    return run_test()


def main() -> int:
    revs = subprocess.check_output(
        ["git", "-C", str(ROOT), "rev-list", "--first-parent", PIN],
        text=True,
    ).splitlines()
    if not revs:
        return 1
    first_good: int | None = None
    last_bad: int | None = None
    seen: set[int] = set()
    for jump in JUMPS:
        if jump >= len(revs):
            jump = len(revs) - 1
        if jump in seen:
            continue
        seen.add(jump)
        rc = probe(revs, jump)
        if rc == 0:
            first_good = jump
            break
        if rc == 1:
            last_bad = jump
    if first_good is None:
        print("no good commit", file=sys.stderr)
        return 1
    lo = (last_bad + 1) if last_bad is not None else 0
    hi = first_good
    best_idx = first_good
    while lo < hi:
        mid = (lo + hi) // 2
        if probe(revs, mid) == 0:
            best_idx = mid
            hi = mid
        else:
            lo = mid + 1
    print(revs[best_idx])
    subprocess.check_call(["git", "-C", str(ROOT), "log", "-1", "--oneline", revs[best_idx]])
    subprocess.check_call(["git", "-C", str(ROOT), "checkout", "-q", "work/ledger-bisect-part-31"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
