#!/usr/bin/env python3
"""Classify one witness run vs fixed PIN ledger and PIN broken observation."""
from __future__ import annotations

import re
import sys
from pathlib import Path

PIN_LEDGER = Path("/tmp/ledger-bisect-pin/corpus_outcomes.txt")
PIN_RECORD = Path("/tmp/ledger-bisect-pin/witness_pin_record.txt")


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_fixture_record(text: str, fixture: str) -> tuple[str, list[str]] | None:
    status = None
    diags: list[str] = []
    for line in text.splitlines():
        if line.startswith(fixture + " "):
            parts = line.split()
            if len(parts) >= 2:
                status = parts[1]
            continue
        if status is not None and line.startswith("\t"):
            diags.append(line[1:].strip())
            continue
        if status is not None and line.strip() and not line.startswith("\t"):
            break
    if status is None:
        return None
    return status, diags


def ledger_expected(fixture: str) -> tuple[str, str | None]:
    for line in PIN_LEDGER.read_text().splitlines():
        if not line.startswith(fixture + " "):
            continue
        parts = line.split()
        status = parts[1]
        for p in parts:
            if p.startswith("regressed-from:"):
                return p.split(":", 1)[1], p.split(":", 1)[1]
        return status, None
    raise SystemExit(f"missing fixture in pin ledger: {fixture}")


def main() -> int:
    witness = sys.argv[1]
    rec_path = Path(sys.argv[2])
    if not rec_path.exists():
        return 125
    observed = parse_fixture_record(rec_path.read_text(), witness)
    pin_broken = parse_fixture_record(PIN_RECORD.read_text(), witness)
    if observed is None or pin_broken is None:
        return 125
    obs_status, obs_diags = observed
    pin_status, pin_diags = pin_broken
    expected, _ = ledger_expected(witness)
    tier = witness.split("/")[0]

    if tier == "fail":
        expected_set = {"rejected", "incomplete"}
    else:
        expected_set = {expected}

    if obs_status in expected_set:
        return 0

    if obs_status == pin_status:
        pin_norm = norm_diag(pin_diags[0]) if pin_diags else ""
        obs_norm = norm_diag(obs_diags[0]) if obs_diags else ""
        if pin_norm == obs_norm:
            return 1

    return 125


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("usage: bisect_test.py <witness> <record-file>")
    raise SystemExit(main())
