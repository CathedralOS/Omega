#!/usr/bin/env python3
"""Compare one corpus record against fixed PIN ledger + PIN witness observation."""
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
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    spec = json.loads(SPEC.read_text())
    text = rec_path.read_text() if rec_path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return 125
    fixture, status = parse_header(line)
    if fixture != witness:
        return 125
    tier = fixture.split("/")[0]
    try:
        ledger_status, flags = ledger_row(fixture)
    except KeyError:
        return 125
    diags = [l[1:].strip() for l in text.splitlines() if l.startswith("\t")]
    first_diag = diags[0] if diags else ""
    ok_statuses = expected_success_status(flags, ledger_status, tier)

    if status in ok_statuses:
        return 0

    pin_status = spec["pin_status"]
    pin_diag = spec["pin_first_diag"]
    if status != pin_status:
        return 125
    if not pin_diag:
        return 1
    if norm_diag(first_diag) == norm_diag(pin_diag):
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
