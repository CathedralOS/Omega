#!/usr/bin/env python3
"""Walk first-parent older than known_bad until witness meets fixed PIN ledger."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
SCRIPT = Path("/tmp/part14-bisect-tools/tools/bisect_pin_witness.sh")
MAX_STEPS = int(os.environ.get("PIN_GOOD_MAX_STEPS", "80"))


def run_test(commit: str, witness: str) -> int:
    subprocess.run(["git", "-C", str(ROOT), "checkout", "-f", commit], check=True, capture_output=True)
    env = {**os.environ, "BISECT_WITNESS": witness, "CARGO_TARGET_DIR": "/tmp/omega-bisect-target-" + commit[:8]}
    r = subprocess.run(["bash", str(SCRIPT)], env=env, capture_output=True)
    return r.returncode


def main() -> int:
    witness, pin, branch = sys.argv[1], sys.argv[2], sys.argv[3]
    known_bad = sys.argv[4] if len(sys.argv) > 4 else "741270df60f7eab218e879eb6771d2bc75e64c00"
    subprocess.run(["git", "-C", str(ROOT), "checkout", "-f", branch], check=True, capture_output=True)
    pin_rc = run_test(pin, witness)
    if pin_rc != 1:
        print(f"PIN not bad rc={pin_rc}", file=sys.stderr)
        subprocess.run(["git", "-C", str(ROOT), "checkout", "-f", branch], check=True)
        return 2
    commits = (
        subprocess.check_output(
            ["git", "-C", str(ROOT), "rev-list", "--first-parent", pin],
            text=True,
        )
        .strip()
        .splitlines()
    )
    start = 1
    if known_bad in commits:
        start = commits.index(known_bad) + 1
    stop = min(len(commits), start + MAX_STEPS)
    for i in range(start, stop):
        rc = run_test(commits[i], witness)
        if rc == 0:
            subprocess.run(["git", "-C", str(ROOT), "checkout", "-f", branch], check=True)
            print(commits[i])
            return 0
    # widen search if still nothing
    for i in range(stop, min(len(commits), start + 300)):
        rc = run_test(commits[i], witness)
        if rc == 0:
            subprocess.run(["git", "-C", str(ROOT), "checkout", "-f", branch], check=True)
            print(commits[i])
            return 0
    subprocess.run(["git", "-C", str(ROOT), "checkout", "-f", branch], check=True)
    print("no good commit found", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
