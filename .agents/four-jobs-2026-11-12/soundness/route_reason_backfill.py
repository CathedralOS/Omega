#!/usr/bin/env python3
"""Backfill route_reason for legacy TSV rows (pre soundness_fix2)."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

_REPO = Path(__file__).resolve().parents[3]
_ROUTE = _REPO / "target/debug/examples/route_one_comparison"


def fetch_route_reason(mutant: Path) -> str:
    if not _ROUTE.is_file() or not mutant.is_file():
        return ""
    try:
        proc = subprocess.run(
            [str(_ROUTE), str(mutant)],
            cwd=_REPO,
            capture_output=True,
            text=True,
            timeout=120,
            stdin=subprocess.DEVNULL,
        )
    except subprocess.TimeoutExpired:
        return "timeout"
    for line in proc.stdout.splitlines():
        if line.startswith("program\t"):
            fields = line.split("\t")
            for i, f in enumerate(fields):
                if f.startswith("route="):
                    tail = [x for x in fields[i + 1 :] if x.strip()]
                    return " ".join(tail)[:500]
    return ""


if __name__ == "__main__":
    print(fetch_route_reason(Path(sys.argv[1])))
