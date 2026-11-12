#!/usr/bin/env python3
"""Bisect test against fixed PIN ledger + PIN witness observation (not per-rev ledger)."""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 2:
        return None
    return parts[0], parts[1]


def parse_record(text: str, fixture: str) -> tuple[str | None, str]:
    status = None
    diags: list[str] = []
    current = None
    for line in text.splitlines():
        if line.startswith("\t") and current == fixture:
            diags.append(line[1:].strip())
            continue
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) < 2:
            continue
        if not parts[0].startswith(("pass/", "fail/", "run/")):
            continue
        current = parts[0]
        if current == fixture:
            status = parts[1]
            diags = []
    first_diag = diags[0] if diags else ""
    return status, first_diag


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def ledger_expected_status(ledger_text: str, fixture: str) -> str | None:
    for line in ledger_text.splitlines():
        if line.startswith(fixture + " "):
            return line.split()[1]
    return None


def status_rank(tier: str, status: str) -> int:
    if tier == "fail":
        if status in ("rejected", "incomplete"):
            return 2
        return 1
    rank = {"built": 3, "checked": 2, "rejected": 1, "incomplete": 1, "timeout": 0}
    return rank.get(status, 0)


def meets_ledger_expectation(tier: str, observed: str, expected: str) -> bool:
    if tier == "fail":
        return observed in ("rejected", "incomplete")
    return status_rank(tier, observed) >= status_rank(tier, expected)


def main() -> int:
    if len(sys.argv) != 3:
        return 125
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    pin_path = Path(os.environ.get("BISECT_PIN_WITNESS_JSON", "/tmp/ledger_bisect_witness_pin.json"))
    ledger_path = Path(os.environ.get("BISECT_PIN_LEDGER", "/tmp/ledger_bisect_corpus_outcomes.txt"))
    if not pin_path.is_file() or not ledger_path.is_file():
        return 125
    pin = json.loads(pin_path.read_text())
    if pin.get("fixture") != witness:
        return 125
    text = rec_path.read_text() if rec_path.exists() else ""
    obs_status, obs_diag = parse_record(text, witness)
    if obs_status is None:
        return 125
    tier = witness.split("/")[0]
    ledger_text = ledger_path.read_text()
    expected = ledger_expected_status(ledger_text, witness)
    if expected is None:
        return 125

    pin_status = pin["pin_status"]
    pin_diag_norm = pin["pin_diag_norm"]
    obs_diag_norm = norm_diag(obs_diag)

    if meets_ledger_expectation(tier, obs_status, expected):
        return 0

  # Regression: reproduces PIN broken outcome and diagnostic class
    if obs_status == pin_status and (
        not pin_diag_norm or obs_diag_norm == pin_diag_norm or pin_diag_norm in obs_diag_norm
    ):
        return 1

    return 125


if __name__ == "__main__":
    raise SystemExit(main())
