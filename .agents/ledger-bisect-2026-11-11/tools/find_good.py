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
    max_steps = int(os.environ.get("BISECT_GOOD_MAX", "48"))

    commits = subprocess.check_output(
        ["git", "rev-list", "--first-parent", "-n", str(max_steps), PIN],
        cwd=repo,
        text=True,
    ).splitlines()
    if not commits:
        print(PIN)
        return 0

    branch = subprocess.check_output(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd=repo, text=True).strip()

    # commits[0] is newest (PIN); search for newest good (rc==0).
    def test_at(i: int) -> int:
        subprocess.check_call(["git", "checkout", "-q", commits[i]], cwd=repo)
        return run_test(repo, script, witness)

    pin_rc = test_at(0)
    if pin_rc == 0:
        subprocess.check_call(["git", "checkout", "-q", branch], cwd=repo)
        print(commits[0])
        return 0

    lo, hi = 0, len(commits) - 1
    good_idx = None
    while lo <= hi:
        mid = (lo + hi) // 2
        rc = test_at(mid)
        if rc == 0:
            good_idx = mid
            lo = mid + 1  # seek newer still-good
        else:
            hi = mid - 1

    subprocess.check_call(["git", "checkout", "-q", branch], cwd=repo)
    if good_idx is None:
        print(commits[-1])
        return 1
    print(commits[good_idx])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
