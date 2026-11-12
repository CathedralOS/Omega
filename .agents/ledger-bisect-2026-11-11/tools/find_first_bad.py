#!/usr/bin/env python3
"""Binary search first-bad commit with pin_bisect_witness.sh (skips ledger-only)."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

TOOL = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOL))
from repo_root import repo_root  # noqa: E402

ROOT = repo_root()
WITNESS = os.environ.get("BISECT_WITNESS", "")
SCRIPT = TOOL / "bisect_witness.sh"


def is_ledger_only(rev: str) -> bool:
    out = subprocess.check_output(
        ["git", "diff", "--name-only", f"{rev}^", rev],
        cwd=ROOT,
        text=True,
    )
    files = [ln for ln in out.splitlines() if ln.strip()]
    if not files:
        return True
    for f in files:
        if f == "tests/omega/corpus_outcomes.txt":
            continue
        if f.endswith(".md"):
            continue
        return False
    return True


def test_rev(rev: str) -> int:
    if is_ledger_only(rev):
        return 125
    subprocess.check_call(["git", "checkout", "-q", rev], cwd=ROOT)
    env = os.environ.copy()
    env["BISECT_WITNESS"] = WITNESS
    return subprocess.call(["bash", str(SCRIPT)], cwd=ROOT, env=env)


def main() -> int:
    if len(sys.argv) != 4:
        print("usage: find_first_bad.py <good> <bad> <log>", file=sys.stderr)
        return 2
    good, bad, log_path = sys.argv[1:4]
    raw = subprocess.check_output(
        ["git", "rev-list", "--first-parent", f"{good}..{bad}"],
        cwd=ROOT,
        text=True,
    ).splitlines()
    revs = [r for r in reversed(raw) if not is_ledger_only(r)]
    if not revs:
        print("empty range", file=sys.stderr)
        return 2
    log: list[str] = []
    lo, hi = 0, len(revs) - 1
    first_bad = None
    while lo <= hi:
        mid = (lo + hi) // 2
        rev = revs[mid]
        subj = subprocess.check_output(
            ["git", "log", "-1", "--format=%s", rev], cwd=ROOT, text=True
        ).strip()
        ec = test_rev(rev)
        log.append(f"probe {rev[:12]} exit={ec} {subj}")
        print(log[-1], flush=True)
        if ec == 125:
            # Skip ledger-only: treat as good for boundary search, move right
            lo = mid + 1
            continue
        if ec == 1:
            first_bad = rev
            hi = mid - 1
        elif ec == 0:
            lo = mid + 1
        else:
            log.append(f"abort at {rev}: unexpected exit {ec}")
            break
    Path(log_path).write_text("\n".join(log) + "\n")
    if first_bad:
        print(first_bad)
        return 0
    print("no first bad found", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
