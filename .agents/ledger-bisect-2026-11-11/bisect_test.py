#!/usr/bin/env python3
"""Bisect against fixed PIN ledger + PIN broken observation (not per-revision ledger)."""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

PIN_LEDGER = Path(os.environ.get("PIN_LEDGER", "/tmp/pin_corpus_outcomes.txt"))
PIN_EXPECT = Path(os.environ.get("PIN_BISECT_EXPECT", "/tmp/pin_bisect_expect.json"))


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_records(text: str) -> dict[str, dict]:
    records: dict[str, dict] = {}
    current_key: str | None = None
    for line in text.splitlines():
        if line.startswith("\t") and current_key:
            records[current_key]["diagnostics"].append(line[1:].strip())
            continue
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) < 3 or not parts[2].endswith("ms"):
            continue
        fixture, status = parts[0], parts[1]
        records[fixture] = {
            "fixture": fixture,
            "status": status,
            "diagnostics": [],
            "flags": [p for p in parts[3:] if not p.startswith("@") and not p.endswith("ms")],
        }
        current_key = fixture
    return records


def expected_status(flags: list[str], default: str) -> str:
    for f in flags:
        if f.startswith("regressed-from:"):
            return f.split(":", 1)[1]
    return default


def ledger_row(fixture: str) -> dict | None:
    for line in PIN_LEDGER.read_text().splitlines():
        if line.startswith(fixture + " "):
            parts = line.split()
            status = parts[1]
            flags = [p for p in parts[3:] if not p.startswith("@") and not p.endswith("ms")]
            return {"status": status, "flags": flags}
    return None


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    pin = json.loads(PIN_EXPECT.read_text())
    spec = pin[witness]
    text = rec_path.read_text() if rec_path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return 125
    observed = parse_records(text)[witness]
    obs_status = observed["status"]
    obs_diag = (observed["diagnostics"] or [""])[0]
    obs_norm = norm_diag(obs_diag)

    row = ledger_row(witness)
    if row is None:
        return 125
    good_status = expected_status(row["flags"], row["status"])

    tier = witness.split("/")[0]
    if obs_status == spec["bad_status"] and obs_norm == spec["bad_diag_norm"]:
        return 1

    def pass_satisfies(expected: str, actual: str) -> bool:
        order = {"built": 3, "checked": 2, "rejected": 1, "incomplete": 0, "crashed": -1, "timeout": -1}
        return order.get(actual, -2) >= order.get(expected, 0)

    def fail_satisfies(expected: str, actual: str) -> bool:
        return actual in ("rejected", "incomplete") if expected in ("rejected", "incomplete") else actual == expected

    if tier == "fail":
        if fail_satisfies(good_status, obs_status):
            return 0
    elif pass_satisfies(good_status, obs_status):
        return 0
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
