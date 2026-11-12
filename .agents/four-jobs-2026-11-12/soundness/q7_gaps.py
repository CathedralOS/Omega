"""Load Q7 mut-migration gap sites from PR #2638 audit (read-only)."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
_REF = "origin/pr-2638"
_LINE1 = re.compile(
    r"^- `(?P<path>tests/omega/[^`]+\.omg)`:(?P<line>\d+) `(?P<name>[A-Za-z_][A-Za-z0-9_]*)`"
)
_LINE2 = re.compile(
    r"^- `(?P<path>tests/omega/[^`]+\.omg)`:(?P<line>\d+) — `(?P<name>[A-Za-z_][A-Za-z0-9_]*)`"
)


def load_q7_gap_sites() -> set[tuple[str, str]]:
    try:
        text = subprocess.check_output(
            ["git", "-C", str(REPO), "show", f"{_REF}:wiki/drafts/audits/mut_migration_2026-11-12.md"],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except subprocess.CalledProcessError:
        return set()
    out: set[tuple[str, str]] = set()
    for line in text.splitlines():
        for pat in (_LINE1, _LINE2):
            m = pat.match(line.strip())
            if m:
                out.add((m.group("path"), m.group("name")))
    return out
