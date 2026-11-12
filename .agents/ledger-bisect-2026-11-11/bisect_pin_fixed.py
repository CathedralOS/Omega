#!/usr/bin/env python3
"""Bisect against frozen PIN ledger and witness observation."""
from __future__ import annotations

import os
import re
import sys
from pathlib import Path

FROZEN_LEDGER = Path(os.environ.get("PIN_FROZEN_LEDGER", "/tmp/pin_corpus_outcomes.txt"))
PIN_OBS = Path(os.environ.get("PIN_WITNESS_OBS", "/tmp/pin_witness_observation.txt"))
RECORD = Path(os.environ.get("BISECT_RECORD", "/tmp/bisect_one_record.txt"))


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_pin_obs() -> tuple[str, str, str, str]:
    data: dict[str, str] = {}
    for line in PIN_OBS.read_text().splitlines():
        if "=" in line:
            k, v = line.split("=", 1)
            data[k.strip()] = v.strip()
    witness = data["witness"]
    return witness, data["pin_status"], data.get("pin_diag", ""), norm_diag(data.get("pin_diag", ""))


def ledger_expectation(fixture: str) -> tuple[str | None, str | None]:
    regressed_from: str | None = None
    status: str | None = None
    for line in FROZEN_LEDGER.read_text().splitlines():
        if not line.startswith(fixture + " "):
            continue
        parts = line.split()
        status = parts[1]
        for p in parts[3:]:
            if p.startswith("regressed-from:"):
                regressed_from = p.split(":", 1)[1]
        return status, regressed_from
    return None, None


def parse_record(fixture: str) -> tuple[str | None, str]:
    if not RECORD.exists():
        return None, ""
    text = RECORD.read_text()
    diag = ""
    status: str | None = None
    in_fixture = False
    for line in text.splitlines():
        if line.startswith(fixture + " "):
            status = line.split()[1]
            in_fixture = True
            continue
        if in_fixture:
            if line.startswith("\t"):
                diag = line[1:].strip()
                break
            if line and not line[0].isspace():
                break
    return status, diag


def meets_expectation(tier: str, expected: str, observed: str, regressed_from: str | None) -> bool:
    if regressed_from:
        base = regressed_from.split("+", 1)[0]
        rank = {"built": 3, "checked": 2, "incomplete": 1, "rejected": 0}
        if tier == "fail":
            return observed in ("rejected", "incomplete")
        want = rank.get(base, 2)
        return rank.get(observed, 0) >= want
    if tier == "fail":
        return observed in ("rejected", "incomplete")
    rank = {"built": 3, "checked": 2, "incomplete": 1, "rejected": 0}
    want = rank.get(expected, 2)
    return rank.get(observed, 0) >= want


def matches_pin_break(pin_status: str, pin_norm: str, observed: str, diag: str) -> bool:
    if observed != pin_status:
        return False
    if not pin_norm:
        return True
    return norm_diag(diag) == pin_norm


def main() -> int:
    witness, pin_status, _pin_diag, pin_norm = parse_pin_obs()
    tier = witness.split("/")[0]
    expected, regressed_from = ledger_expectation(witness)
    if expected is None:
        return 125
    observed, diag = parse_record(witness)
    if observed is None:
        return 125
    if os.environ.get("BISECT_CLASS_ONLY") == "1":
        return 1 if matches_pin_break(pin_status, pin_norm, observed, diag) else 0
    if matches_pin_break(pin_status, pin_norm, observed, diag):
        return 1
    if meets_expectation(tier, expected, observed, regressed_from):
        return 0
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
