#!/usr/bin/env python3
"""Resolve a ledger @ stamp or last known-good first-parent commit for a fixture."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
GOLDEN = REPO / "tests/omega/corpus_outcomes.txt"


def disambiguate(short: str) -> str | None:
    for prefix in (short, short[:12], short[:8]):
        r = subprocess.run(
            ["git", "-C", str(REPO), "rev-parse", "--verify", f"{prefix}^{{commit}}"],
            capture_output=True,
            text=True,
        )
        if r.returncode == 0:
            return r.stdout.strip()
    r = subprocess.run(
        ["git", "-C", str(REPO), "rev-list", "--all", "--abbrev-commit"],
        capture_output=True,
        text=True,
    )
    for line in r.stdout.splitlines():
        if line.startswith(short) or short.startswith(line.split()[0]):
            full = line.split()[0]
            r2 = subprocess.run(
                ["git", "-C", str(REPO), "rev-parse", f"{full}^{{commit}}"],
                capture_output=True,
                text=True,
            )
            if r2.returncode == 0:
                return r2.stdout.strip()
    return None


def ledger_stamp(fixture: str) -> str | None:
    for line in GOLDEN.read_text().splitlines():
        if not line.startswith(fixture + " "):
            continue
        for tok in line.split():
            if tok.startswith("@") and len(tok) > 1:
                return disambiguate(tok[1:])
        break
    return None


def last_status_flip_commit(fixture: str, want: str = "checked") -> str | None:
    """Last first-parent commit that still records the fixture at `want` in the ledger."""
    r = subprocess.run(
        [
            "git",
            "-C",
            str(REPO),
            "log",
            "--first-parent",
            "-S",
            f"{fixture} {want}",
            "-1",
            "--format=%H",
            "--",
            "tests/omega/corpus_outcomes.txt",
        ],
        capture_output=True,
        text=True,
    )
    return r.stdout.strip() or None


def main() -> int:
    fixture = sys.argv[1]
    good = ledger_stamp(fixture)
    if good is None:
        good = last_status_flip_commit(fixture)
    if good is None:
        print("", end="")
        return 1
    print(good)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
