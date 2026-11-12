#!/usr/bin/env python3
"""Exponential first-parent jump trace from PIN (build/run at each jump)."""
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
    seen: set[int] = set()
    for jump in JUMPS:
        if jump >= len(revs):
            jump = len(revs) - 1
        if jump in seen:
            continue
        seen.add(jump)
        subprocess.check_call(
            ["git", "-C", str(ROOT), "checkout", "-q", "-f", revs[jump]],
        )
        rc = run_test(witness)
        print(f"  jump idx={jump} sha={revs[jump]} exit={rc}", flush=True)
    subprocess.check_call(["git", "-C", str(ROOT), "checkout", "-q", "work/ledger-bisect-part-31"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
