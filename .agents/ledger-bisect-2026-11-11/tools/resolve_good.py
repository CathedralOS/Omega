#!/usr/bin/env python3
"""Resolve git-bisect good commit for a witness from corpus_outcomes."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]


def main() -> int:
    fix = sys.argv[1]
    ledger = (REPO / "tests/omega/corpus_outcomes.txt").read_text().splitlines()
    row = next((ln for ln in ledger if ln.startswith(fix + " ")), "")
    tier = fix.split("/")[0]
    if tier == "fail":
        want = "rejected"
        if "incomplete" in row.split():
            want = "incomplete"
    else:
        parts = row.split()
        if len(parts) > 1 and parts[1] in ("checked", "built"):
            want = parts[1]
        elif "built" in row:
            want = "built"
        else:
            want = "checked"
    needle = f"{fix} {want}"
    out = subprocess.run(
        ["git", "log", "-1", f"-S{needle}", "--format=%H", "--", "tests/omega/corpus_outcomes.txt"],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=False,
    )
    good = out.stdout.strip()
    if not good:
        for tok in row.split():
            if tok.startswith("@"):
                full = subprocess.run(
                    ["git", "rev-parse", tok[1:]],
                    cwd=REPO,
                    capture_output=True,
                    text=True,
                    check=False,
                ).stdout.strip()
                if full:
                    good = full
                    break
    if not good:
        good = "759c80404428"
    print(good)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
