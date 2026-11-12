#!/usr/bin/env python3
"""Binary-search first-parent history for newest commit where witness matches ledger."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / ".agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"
PIN = "51e8c45c793007f211d0d9baee7de16767172444"


def run_witness(witness: str, rev: str) -> int:
    subprocess.run(["git", "checkout", "-q", rev], cwd=ROOT, check=True)
    subprocess.run(["chmod", "-R", "u+w", str(ROOT / "target")], check=False)
    env = os.environ.copy()
    env.update(
        {
            "BISECT_WITNESS": witness,
            "CARGO_PROFILE_DEV_OPT_LEVEL": "1",
            "CARGO_TARGET_DIR": str(ROOT / "target"),
            "OMEGA_CORPUS_JOBS": "1",
            "CARGO_BUILD_JOBS": "4",
            "MBX_SCHEDULER_CPUS": "4",
        }
    )
    return subprocess.run(
        ["nice", "-n", "19", str(SCRIPT)], cwd=ROOT, env=env
    ).returncode


def intro(witness: str) -> str:
    rel = f"tests/omega/{witness}/build.omg"
    lines = subprocess.check_output(
        ["git", "log", "--reverse", "--format=%H", "--", rel],
        cwd=ROOT,
        text=True,
    ).splitlines()
    if not lines:
        raise SystemExit(f"no intro for {witness}")
    return lines[0]


def first_parent_path(old: str, new: str) -> list[str]:
    """Return first-parent commits from old (inclusive) to new (inclusive), oldest first."""
    out: list[str] = []
    cur = new
    while True:
        out.append(cur)
        if cur == old:
            break
        cur = subprocess.check_output(
            ["git", "rev-parse", f"{cur}^"], cwd=ROOT, text=True
        ).strip()
    out.reverse()
    return out


def main() -> int:
    witness = sys.argv[1]
    lo = intro(witness)
    hi = PIN
    path = first_parent_path(lo, hi)
    # Ensure endpoints: lo should be good (0), hi bad (1)
    lo_ec = run_witness(witness, path[0])
    hi_ec = run_witness(witness, path[-1])
    if lo_ec != 0:
        # search subpath from lo forward
        for i, rev in enumerate(path[:30]):
            if run_witness(witness, rev) == 0:
                path = path[i:]
                break
        else:
            return 1
    if hi_ec != 1:
        print(f"pin not worse than ledger ec={hi_ec}", file=sys.stderr)
        return 1
    lo_idx, hi_idx = 0, len(path) - 1
    while hi_idx - lo_idx > 1:
        mid = (lo_idx + hi_idx) // 2
        ec = run_witness(witness, path[mid])
        if ec == 0:
            lo_idx = mid
        elif ec == 1:
            hi_idx = mid
        else:
            # skip ambiguous 125: bias older
            hi_idx = mid
    subprocess.run(["git", "checkout", "-q", PIN], cwd=ROOT, check=True)
    print(path[lo_idx])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
