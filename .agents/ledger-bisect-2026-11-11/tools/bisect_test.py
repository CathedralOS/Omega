#!/usr/bin/env python3
"""Compare one fixture run against fixed PIN ledger + PIN broken observation."""
from __future__ import annotations

import os
import re
import sys
from pathlib import Path

PIN_LEDGER = Path(os.environ["OMEGA_PIN_LEDGER"])
PIN_OBS = Path(os.environ["OMEGA_PIN_OBS"])


def ledger_expected(fixture: str) -> tuple[str | None, str | None]:
    regressed_from = None
    for line in PIN_LEDGER.read_text().splitlines():
        if not line.startswith(fixture + " "):
            continue
        parts = line.split()
        status = parts[1]
        for p in parts[2:]:
            if p.startswith("regressed-from:"):
                regressed_from = p.split(":", 1)[1]
        return status, regressed_from
    return None, None


def parse_record(text: str, fixture: str) -> tuple[str | None, str, list[str]]:
    header = next((l for l in text.splitlines() if l.startswith(fixture + " ")), None)
    if not header:
        return None, "", []
    status = header.split()[1]
    diags: list[str] = []
    past = False
    for line in text.splitlines():
        if line == header:
            past = True
            continue
        if not past:
            continue
        if line.startswith("\t"):
            diags.append(line[1:].strip())
        elif line.strip():
            break
    return fixture, status, diags


def pin_broken(witness: str) -> tuple[str, str]:
    _, status, diags = parse_record(PIN_OBS.read_text(), witness)
    return status, diags[0] if diags else ""


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def pass_ok(observed: str, expected: str, regressed_from: str | None) -> bool:
    target = regressed_from if regressed_from else expected
    if target in ("checked", "built"):
        return observed in ("checked", "built")
    return observed == target


def main() -> int:
    witness = os.environ["BISECT_WITNESS"]
    rec_path = Path(sys.argv[1])
    text = rec_path.read_text() if rec_path.exists() else ""
    fixture, observed, diags = parse_record(text, witness)
    if not fixture or not observed:
        return 125

    expected, regressed_from = ledger_expected(fixture)
    if expected is None:
        return 125

    pin_status, pin_first = pin_broken(witness)
    obs_first = diags[0] if diags else ""
    if observed == pin_status and norm_diag(obs_first) == norm_diag(pin_first):
        return 1

    tier = fixture.split("/")[0]
    if tier == "fail":
        exp = regressed_from or expected
        if exp in ("rejected", "incomplete") and observed == exp:
            return 0
        return 125

    if pass_ok(observed, expected, regressed_from):
        return 0
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
