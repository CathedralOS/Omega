#!/usr/bin/env python3
"""Compare one corpus record against PIN-frozen witness expectation."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

PIN_DIR = Path("/tmp/bisect-pin")


def parse_header(line: str) -> tuple[str, str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1], parts[0].split("/")[0]


def first_diag(text: str, fixture: str) -> str:
    in_rec = False
    for line in text.splitlines():
        if line.startswith(fixture + " "):
            in_rec = True
            continue
        if in_rec:
            if line.startswith("\t"):
                return line[1:].strip()
            break
    return ""


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    specs = json.loads((PIN_DIR / "witnesses.json").read_text())
    if witness not in specs:
        return 125
    spec = specs[witness]
    text = rec_path.read_text() if rec_path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return 125
    parsed = parse_header(line)
    if not parsed:
        return 125
    fixture, status, tier = parsed
    if fixture != witness:
        return 125
    good = spec["good_status"]
    if status == good:
        return 0
    diag = first_diag(text, witness)
    if tier == "fail":
        if good in ("rejected", "incomplete") and status not in ("rejected", "incomplete"):
            return 1
        return 125
    # pass/run: ledger expects compile success class
    if good in ("checked", "built"):
        if status in ("rejected", "incomplete"):
            if norm_diag(diag) == spec["bad_diag_norm"]:
                return 1
            # still a pass regression for bisect narrowing
            return 1
        return 125
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
