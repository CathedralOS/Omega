#!/usr/bin/env python3
"""Exponential first-parent walk from PIN; newest commit (min index) where witness exits 0."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
ROOT = Path(__file__).resolve().parents[2]
SCRIPT = Path("/tmp/ledger-bisect-pin/tools/bisect_witness.sh")
JUMPS = [0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384]


def run_test(witness: str) -> int:
    env = {**dict(**__import__("os").environ), "BISECT_WITNESS": witness}
    return subprocess.call(["bash", str(SCRIPT)], cwd=ROOT, env=env)


def main() -> int:
    witness = sys.argv[1]
    revs = subprocess.check_output(
        ["git", "-C", str(ROOT), "rev-list", "--first-parent", PIN], text=True
    ).splitlines()
    print(f"witness={witness} pin={PIN} commits={len(revs)}", flush=True)
    last_bad: int | None = None
    first_good: int | None = None
    seen: set[int] = set()
    for jump in JUMPS:
        if jump >= len(revs):
            jump = len(revs) - 1
        if jump in seen:
            continue
        seen.add(jump)
        subprocess.check_call(["git", "-C", str(ROOT), "checkout", "-q", "-f", revs[jump]])
        rc = run_test(witness)
        print(f"  jump idx={jump} sha={revs[jump]} exit={rc}", flush=True)
        if rc == 0 and first_good is None:
            first_good = jump
        if rc == 1:
            last_bad = jump if last_bad is None or jump < last_bad else last_bad

  # Newest good = minimum index with exit 0 (closest to PIN).
    lo = 0
    hi = len(revs) - 1
    best: int | None = None
    while lo <= hi:
        mid = (lo + hi) // 2
        subprocess.check_call(["git", "-C", str(ROOT), "checkout", "-q", "-f", revs[mid]])
        rc = run_test(witness)
        print(f"  newest-good idx={mid} sha={revs[mid]} exit={rc}", flush=True)
        if rc == 0:
            best = mid
            hi = mid - 1
        elif rc == 1:
            lo = mid + 1
        else:
            # Build/skip: step toward newer commits first (often toolchain drift on ancient revs).
            hi = mid - 1
    if best is None:
        print("NO_GOOD", flush=True)
        subprocess.check_call(["git", "-C", str(ROOT), "checkout", "-q", "work/ledger-bisect-part-31"])
        return 1
    subprocess.check_call(["git", "-C", str(ROOT), "checkout", "-q", "-f", revs[best]])
    rc = run_test(witness)
    print(f"  confirm idx={best} sha={revs[best]} exit={rc}", flush=True)
    print(f"NEWEST_GOOD_IDX={best}", flush=True)
    print(f"NEWEST_GOOD_SHA={revs[best]}", flush=True)
    if first_good is not None:
        print(f"EXPONENTIAL_FIRST_GOOD_JUMP_IDX={first_good} sha={revs[first_good]}", flush=True)
    if last_bad is not None:
        print(f"EXPONENTIAL_LAST_BAD_JUMP_IDX={last_bad} sha={revs[last_bad]}", flush=True)
    subprocess.check_call(["git", "-C", str(ROOT), "checkout", "-q", "work/ledger-bisect-part-31"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
