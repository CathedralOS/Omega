#!/usr/bin/env python3
"""Find newest first-parent ancestor of PIN where witness is good (fixed PIN ledger)."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
PIN = os.environ.get("PIN_COMMIT", "51e8c45c793007f211d0d9baee7de16767172444")
WITNESS = os.environ["BISECT_WITNESS"]


def run_test(root: Path, rev: str) -> int:
    subprocess.check_call(
        ["git", "-C", str(root), "checkout", "-f", rev],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    env = {**os.environ, "BISECT_WITNESS": WITNESS}
    return subprocess.run(
        ["/tmp/ledger-bisect-tools/bisect_witness.sh"],
        cwd=root,
        env=env,
    ).returncode


def main() -> int:
    root = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
    subprocess.check_call(
        [str(TOOLS / "prepare_bisect_witness.sh"), WITNESS, PIN],
        cwd=root,
    )
    revs = subprocess.check_output(
        ["git", "-C", str(root), "rev-list", "--first-parent", PIN],
        text=True,
    ).strip().splitlines()
    if not revs:
        raise SystemExit("empty rev-list")
    floor = (
        subprocess.check_output(
            [
                "git",
                "-C",
                str(root),
                "log",
                "--first-parent",
                "-1",
                "--format=%H",
                "-S",
                WITNESS,
                "--",
                "tests/omega/corpus_outcomes.txt",
            ],
            text=True,
        ).strip()
        or revs[-1]
    )
    try:
        floor_i = revs.index(floor)
    except ValueError:
        floor_i = len(revs) - 1
    revs = revs[: floor_i + 1]
    print(f"FLOOR={floor} span={len(revs)}", flush=True)
    pin_code = run_test(root, revs[0])
    print(f"PIN[{revs[0][:12]}] exit={pin_code}", flush=True)
    if pin_code != 1:
        print(f"PIN witness not bad at {revs[0]}", file=sys.stderr)
    bad_hi = 0
    good_idx: int | None = None
    step = 1
    last_good: int | None = None
    while step < len(revs):
        code = run_test(root, revs[step])
        print(f"exp[{step}] {revs[step][:12]} exit={code}", flush=True)
        if code == 0:
            last_good = step
            good_idx = step
            break
        if code == 1:
            bad_hi = step
        step = min(step * 2, len(revs) - 1)
    if good_idx is None:
        floor_code = run_test(root, revs[floor_i])
        print(f"floor[{floor_i}] {revs[floor_i][:12]} exit={floor_code}", flush=True)
        if floor_code == 0:
            good_idx = floor_i
    if good_idx is not None and bad_hi and bad_hi + 1 < good_idx:
        lo, hi = bad_hi + 1, good_idx - 1
        while lo <= hi:
            mid = (lo + hi) // 2
            code = run_test(root, revs[mid])
            print(f"bin[{mid}] {revs[mid][:12]} exit={code}", flush=True)
            if code == 0:
                good_idx = mid
                hi = mid - 1
            else:
                lo = mid + 1
    if good_idx is None:
        raise SystemExit("GOOD not found")
    good = revs[good_idx]
    print(f"GOOD={good}")
    Path("/tmp/ledger_bisect_good.txt").write_text(good + "\n")
    subprocess.check_call(["git", "-C", str(root), "checkout", "work/ledger-bisect-part-22"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
