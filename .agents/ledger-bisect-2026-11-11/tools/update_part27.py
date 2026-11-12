#!/usr/bin/env python3
"""Refresh part-27.md row for one cluster from .result file."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PART = ROOT / ".agents/ledger-bisect-2026-11-11/part-27.md"
RES = ROOT / f".agents/ledger-bisect-2026-11-11/bisect-{sys.argv[1]}.result"


def pr_from_commit(sha: str) -> str:
    import subprocess

    subj = subprocess.check_output(["git", "log", "-1", "--format=%s", sha], text=True).strip()
    m = re.search(r"#(\d+)", subj)
    return m.group(1) if m else "none"


def mechanism(sha: str) -> str:
    import subprocess

    subj = subprocess.check_output(["git", "log", "-1", "--format=%s", sha], text=True).strip()
    stat = subprocess.check_output(
        ["git", "show", "--stat", "--format=", sha], text=True
    ).strip().splitlines()[:4]
    tail = "; ".join(s.strip() for s in stat)
    return f"{subj} — {tail}"[:280]


def main() -> int:
    cid = sys.argv[1]
    lines = RES.read_text().splitlines()
    sha = lines[0].strip()
    pr = pr_from_commit(sha)
    mech = mechanism(sha)
    text = PART.read_text()
    # replace breaking commit for cluster row
    pattern = rf"(\| {cid} \|[^\n]+\|) \(pending\)"
    if not re.search(pattern, text):
        pattern = rf"(\| {cid} \|[^\n]+\|) ``"
    repl = rf"\1 `{sha}`"
    text = re.sub(pattern, repl, text, count=1)
    text = re.sub(
        rf"(\| {cid} \|[^\n]+\| `{re.escape(sha)}` \|) [^|]+(\|) [^|]+(\| in progress)",
        rf"\1 {pr}\2 {mech}\3 bisected",
        text,
        count=1,
    )
    PART.write_text(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
