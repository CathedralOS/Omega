#!/usr/bin/env python3
"""Bisect against fixed PIN ledger + PIN broken observation (not checkout ledger)."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

PIN_LEDGER = Path("/tmp/pin_corpus_outcomes.txt")
PIN_OBS = Path("/tmp/pin_witness_obs.json")


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1]


def parse_record_block(text: str, fixture: str) -> dict:
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if not line.startswith(fixture + " "):
            continue
        parts = line.split()
        status = parts[1]
        flags = parts[3:] if len(parts) > 3 else []
        diags: list[str] = []
        j = i + 1
        while j < len(lines) and lines[j].startswith("\t"):
            diags.append(lines[j][1:].strip())
            j += 1
        return {"fixture": fixture, "status": status, "flags": flags, "diagnostics": diags}
    return {}


def ledger_line(fixture: str) -> dict | None:
    if not PIN_LEDGER.exists():
        return None
    rec = parse_record_block(PIN_LEDGER.read_text(), fixture)
    return rec or None


def expected_outcome(fixture: str) -> str | None:
    rec = ledger_line(fixture)
    if not rec:
        return None
    tier = fixture.split("/")[0]
    status = rec["status"]
    flags = rec["flags"]
    for f in flags:
        if f.startswith("regressed-from:"):
            return f.split(":", 1)[1]
    if tier == "fail":
        return status
    return status


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def rank(status: str) -> int:
    return {"built": 2, "checked": 1, "rejected": 0, "incomplete": 0}.get(status, -1)


def worse_than_expected(observed: str, expected: str, tier: str) -> bool:
    if tier == "fail":
        return expected in ("rejected", "incomplete") and observed not in ("rejected", "incomplete")
    return rank(observed) < rank(expected)


def main() -> int:
    witness = sys.argv[1]
    rec_path = Path(sys.argv[2])
    if not PIN_OBS.exists():
        return 125
    expected = expected_outcome(witness)
    if expected is None:
        return 125
    text = rec_path.read_text() if rec_path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return 125
    obs_status = parse_header(line)[1]
    tier = witness.split("/")[0]
    if tier != "fail" and rank(obs_status) >= rank(expected):
        return 0
    if obs_status == expected:
        return 0
    if worse_than_expected(obs_status, expected, tier):
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
