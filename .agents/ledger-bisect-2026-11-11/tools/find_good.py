#!/usr/bin/env python3
"""Find newest first-parent ancestor of PIN where witness matches PIN-ledger expectation."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

PIN = os.environ.get("BISECT_PIN", "51e8c45c793007f211d0d9baee7de16767172444")


def run_test(repo: Path, script: Path, witness: str) -> int:
    env = os.environ.copy()
    env["BISECT_WITNESS"] = witness
    env["OMEGA_REPO_ROOT"] = str(repo)
    return subprocess.run([str(script)], env=env, cwd=repo).returncode


def main() -> int:
    witness = sys.argv[1]
    repo = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
    script = Path(os.environ["BISECT_WITNESS_SCRIPT"])
    hard_max = int(os.environ.get("BISECT_GOOD_MAX", "4000"))

    commits = subprocess.check_output(
        ["git", "rev-list", "--first-parent", "-n", str(hard_max), PIN],
        cwd=repo,
        text=True,
    ).splitlines()
    if not commits:
        print(PIN)
        return 0

    branch = subprocess.check_output(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd=repo, text=True).strip()

    def test_at(i: int) -> int:
        subprocess.check_call(["git", "checkout", "-q", commits[i]], cwd=repo)
        return run_test(repo, script, witness)

    if test_at(0) == 0:
        subprocess.check_call(["git", "checkout", "-q", branch], cwd=repo)
        print(commits[0])
        return 0

    # Exponential probe for oldest index that is still good (newest good = smallest index with rc==0).
    hi_bad = 0
    lo_good = len(commits) - 1
    step = 1
    while step < len(commits):
        i = step
        rc = test_at(i)
        if rc == 0:
            lo_good = i
            break
        hi_bad = i
        step *= 2

    if test_at(lo_good) != 0:
        # No good in window; return deepest ancestor tested.
        subprocess.check_call(["git", "checkout", "-q", branch], cwd=repo)
        print(commits[-1])
        return 1

    # Binary search for smallest index with rc==0 (newest good).
    lo, hi = hi_bad, lo_good
    best = lo_good
    while lo <= hi:
        mid = (lo + hi) // 2
        rc = test_at(mid)
        if rc == 0:
            best = mid
            hi = mid - 1
        else:
            lo = mid + 1

    subprocess.check_call(["git", "checkout", "-q", branch], cwd=repo)
    print(commits[best])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
