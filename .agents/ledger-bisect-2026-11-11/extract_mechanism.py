#!/usr/bin/env python3
"""One-line mechanism from first-parent breaking commit diff (compiler paths only)."""
from __future__ import annotations

import subprocess
import sys


def main() -> int:
    commit, cluster_id = sys.argv[1], sys.argv[2]
    diff = subprocess.check_output(
        ["git", "show", "--stat", "--format=", commit],
        text=True,
    )
    lines = [l.strip() for l in diff.splitlines() if l.strip() and " =>" in l]
    # prefer omega-rust/psi changes
    picks = [l for l in lines if "omega-rust/" in l or "source/" in l]
    if not picks:
        picks = [l for l in lines if "corpus_outcomes" not in l]
    mech = picks[0] if picks else (lines[0] if lines else "unknown")
    if len(picks) > 1:
        mech = f"{mech}; {picks[1]}"
    print(f"mechanism: {mech}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
