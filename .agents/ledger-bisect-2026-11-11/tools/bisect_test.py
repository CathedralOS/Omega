#!/usr/bin/env python3
"""Score one corpus record for git bisect.

BISECT_MODE=outcome (default):
  0 = ledger-expected status for this fixture
  1 = any other observed outcome
  125 = no record (the commit did not run)

BISECT_MODE=class:
  1 = PIN status and PIN diagnostic class
  0 = any other observed outcome
  125 = no record
"""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

LEDGER = Path(os.environ.get("BISECT_LEDGER", "/tmp/ledger-bisect-pin-corpus_outcomes.txt"))
SPEC = Path(os.environ["BISECT_SPEC"])


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 2:
        return None
    return parts[0], parts[1]


def ledger_row(fixture: str) -> tuple[str, list[str]]:
    for line in LEDGER.read_text().splitlines():
        if line.startswith(fixture + " "):
            parts = line.split()
            return parts[1], parts[3:]
    raise KeyError(fixture)


def expected_success_status(flags: list[str], ledger_status: str, tier: str) -> set[str]:
    for tok in flags:
        if tok.startswith("regressed-from:"):
            target = tok.split(":", 1)[1]
            if target == "checked":
                return {"checked", "built"}
            return {target}
    if tier == "fail":
        return {"rejected", "incomplete"}
    if ledger_status in ("checked", "built"):
        return {"checked", "built"}
    return {ledger_status}


def main() -> int:
    mode = os.environ.get("BISECT_MODE", "outcome")
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    spec = json.loads(SPEC.read_text())
    text = rec_path.read_text() if rec_path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return 125
    parsed = parse_header(line)
    if parsed is None or parsed[0] != witness:
        return 125
    fixture, status = parsed
    diags = [l[1:].strip() for l in text.splitlines() if l.startswith("\t")]
    first_diag = diags[0] if diags else ""
    if mode == "class":
        pin_status = spec["pin_status"]
        pin_diag = spec.get("pin_first_diag") or ""
        if status == pin_status and norm_diag(first_diag) == norm_diag(pin_diag):
            return 1
        return 0
    try:
        ledger_status, flags = ledger_row(fixture)
    except KeyError:
        return 125
    tier = fixture.split("/")[0]
    if status in expected_success_status(flags, ledger_status, tier):
        return 0
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
