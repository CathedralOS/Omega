#!/usr/bin/env python3
"""Compare witness run to fixed PIN snapshot (not checkout ledger)."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

BISECT_DIR = Path(
    __import__("os").environ.get(
        "BISECT_FIXTURE_DIR", "/tmp/ledger-bisect-part-8/default"
    )
)


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1]


def first_diagnostic(text: str, fixture: str) -> str:
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if line.startswith(fixture + " "):
            if i + 1 < len(lines) and lines[i + 1].startswith("\t"):
                return lines[i + 1].lstrip("\t").strip()
            return ""
    return ""


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    snap_path = BISECT_DIR / "pin_observation.json"
    if not snap_path.exists():
        return 125
    snap = json.loads(snap_path.read_text())
    if witness != snap["witness"]:
        return 125
    text = rec_path.read_text() if rec_path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return 125
    _, status = parse_header(line)
    diag = first_diagnostic(text, witness)
    diag_norm = norm_diag(diag)

    expected = snap["expected_status"]
    tier = witness.split("/")[0]
    if tier == "pass":
        rank = {"built": 2, "checked": 1}
        if rank.get(status, 0) >= rank.get(expected, 0) and status not in (
            "rejected",
            "incomplete",
        ):
            return 0
    elif status == expected:
        return 0

    broken_status = snap["broken_status"]
    broken_diag = snap["broken_diag"]
    same_class = diag_norm == snap["broken_diag_norm"] or (
        "no executable body" in diag
        and "no executable body" in broken_diag
        and "lower its declaration" in diag
        and "lower" in broken_diag
    )
    if status == broken_status and same_class:
        return 1

    return 125


if __name__ == "__main__":
    raise SystemExit(main())
