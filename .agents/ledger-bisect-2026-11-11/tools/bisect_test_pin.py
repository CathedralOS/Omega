#!/usr/bin/env python3
"""Bisect test using fixed PIN ledger + PIN broken snapshot only."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

CTX = Path("/tmp/ledger-bisect-pin-context.json")


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1]


def first_diag(text: str, fixture: str) -> str:
    seen = False
    for line in text.splitlines():
        if line.startswith(fixture + " "):
            seen = True
            continue
        if seen and line.startswith("\t"):
            return line[1:].strip()
        if seen:
            break
    return ""


def main() -> int:
    witness = sys.argv[1]
    rec_path = Path(sys.argv[2])
    if not CTX.is_file():
        return 125
    ctx = json.loads(CTX.read_text())
    if ctx["witness"] != witness:
        return 125
    text = rec_path.read_text() if rec_path.is_file() else ""
    line = next((ln for ln in text.splitlines() if ln.startswith(witness + " ")), None)
    if not line:
        return 125
    parsed = header(line)
    if not parsed:
        return 125
    fixture, status = parsed
    diag = first_diag(text, fixture)
    broken = ctx["pin_broken"]
    exp = ctx["pin_expected"]
    want = exp["status"]

    def is_good() -> bool:
        if exp["tier"] == "fail":
            if want in ("rejected", "incomplete"):
                return status in ("rejected", "incomplete")
            return status == want
        rank = {"built": 2, "checked": 1}
        return rank.get(status, 0) >= rank.get(want, 0)

    def is_bad() -> bool:
        if status != broken["status"]:
            return False
        if not broken["diag_norm"]:
            return True
        return norm_diag(diag) == broken["diag_norm"]

    if is_good():
        return 0
    if is_bad():
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
