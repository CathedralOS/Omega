#!/usr/bin/env python3
"""Bisect test: compare witness run to fixed PIN ledger + PIN observation."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

PIN_OBS = Path("/tmp/bisect_pin_obs.json")


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def record_for_witness(text: str, witness: str) -> tuple[str, str, str] | None:
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if not line.startswith(witness + " "):
            continue
        parts = line.split()
        if len(parts) < 3 or not parts[2].endswith("ms"):
            return None
        status = parts[1]
        diag = ""
        for j in range(i + 1, len(lines)):
            if lines[j].startswith("\t"):
                diag = lines[j][1:].strip()
                break
            if lines[j].strip() and not lines[j].startswith("\t"):
                break
        return witness, status, diag
    return None


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    if not PIN_OBS.exists():
        return 125
    state = json.loads(PIN_OBS.read_text())
    pin = state.get(witness)
    if not pin:
        return 125
    text = rec_path.read_text() if rec_path.exists() else ""
    parsed = record_for_witness(text, witness)
    if not parsed:
        return 125
    _, status, first_diag = parsed
    good = pin["pin_good_status"]
    bad = pin["pin_bad_status"]
    if status == good:
        return 0
    if status == bad and norm_diag(first_diag) == pin["pin_bad_diag_norm"]:
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
