#!/usr/bin/env python3
"""First-parent bisect good..bad using BISECT_HEAD (no main-repo checkout)."""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path("/agent/repos/omega")
LB = Path("/tmp/ledger-bisect-2026-11-11")


def git(*args: str) -> str:
    return subprocess.check_output(["git", "-C", str(ROOT), *args], text=True).strip()


def ledger_only(commit: str) -> bool:
    try:
        parent = git("rev-parse", f"{commit}^1")
    except subprocess.CalledProcessError:
        return False
    files = git("diff", "--name-only", parent, commit).splitlines()
    if not files:
        return False
    return all(f == "tests/omega/corpus_outcomes.txt" or f.endswith(".md") for f in files)


def step_rc(commit: str, witness: str, step_script: Path) -> int:
    env = {**os.environ, "BISECT_WITNESS": witness, "BISECT_HEAD": commit}
    r = subprocess.run(
        [str(step_script)],
        env=env,
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    print(r.stderr or r.stdout, file=sys.stderr, end="")
    return r.returncode


def main() -> int:
    if len(sys.argv) < 5:
        print("usage: bisect_pin_manual.py WITNESS GOOD BAD OUT.json", file=sys.stderr)
        return 2
    witness, good, bad, out = sys.argv[1:5]
    step_script = Path(sys.argv[5]) if len(sys.argv) > 5 else LB / "tools/bisect_step_pin_diag.sh"
    commits = [c for c in git("rev-list", "--first-parent", "--reverse", f"{good}..{bad}").splitlines() if c]
    if not commits:
        print("empty revision range", file=sys.stderr)
        return 1
    g0 = step_rc(good, witness, step_script)
    b0 = step_rc(bad, witness, step_script)
    print(f"endpoint good {good} rc={g0}", file=sys.stderr)
    print(f"endpoint bad {bad} rc={b0}", file=sys.stderr)
    if g0 != 0 or b0 != 1:
        payload = {
            "witness": witness,
            "good": good,
            "bad": bad,
            "breaking_commit": None,
            "bisect_exit": 1,
            "status": "endpoint_mismatch",
            "endpoint_good_rc": g0,
            "endpoint_bad_rc": b0,
        }
        Path(out).write_text(json.dumps(payload, indent=2))
        return 1
    lo, hi = 0, len(commits) - 1
    first_bad = bad
    while lo <= hi:
        mid = (lo + hi) // 2
        c = commits[mid]
        if ledger_only(c):
            lo = mid + 1
            continue
        rc = step_rc(c, witness, step_script)
        print(f"probe {c} rc={rc}", file=sys.stderr)
        if rc == 0:
            lo = mid + 1
        elif rc == 1:
            first_bad = c
            hi = mid - 1
        else:
            lo = mid + 1
    parent = git("rev-parse", f"{first_bad}^1")
    subj = git("log", "-1", "--format=%s", first_bad)
    pr = None
    m = re.search(r"#(\d+)", subj)
    if m:
        pr = m.group(1)
    stat = subprocess.run(
        ["git", "-C", str(ROOT), "show", "--stat", "--format=", first_bad],
        capture_output=True,
        text=True,
    )
    mech = "\n".join(
        ln
        for ln in stat.stdout.splitlines()
        if not ln.startswith(" tests/omega/corpus_outcomes") and not ln.endswith(".md")
    )[:500]
    payload = {
        "witness": witness,
        "good": good,
        "bad": bad,
        "breaking_commit": first_bad,
        "parent": parent,
        "pr": pr,
        "mechanism": subj[:280],
        "mechanism_detail": mech,
        "bisect_exit": 0,
        "status": "bisected",
    }
    Path(out).write_text(json.dumps(payload, indent=2))
    print(first_bad, pr, subj[:72])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
