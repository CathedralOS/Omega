#!/usr/bin/env python3
"""Compare witness run to fixed PIN observation (not checkout-time ledger)."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

WITNESS_CFG = Path("/tmp/bisect_pin_witness.json")
REC = Path("/tmp/bisect_one_record.txt")


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
        }
        current_key = fixture
    return records


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def infrastructure_skip(text: str) -> bool:
    if not text.strip():
        return True
    if any("failed to read source/library" in d for d in text.splitlines()):
        return True
    if any("captured entry is absent" in d for d in text.splitlines()):
        return True
    line = next((l for l in text.splitlines() if l and not l.startswith("\t")), "")
    parts = line.split()
    if len(parts) >= 3 and parts[2].endswith("ms"):
        try:
            if int(parts[2].rstrip("ms")) < 50:
                return True
        except ValueError:
            pass
    return False


def status_satisfies_expected(observed: str, expected: str) -> bool:
    rank = {"built": 2, "checked": 1, "rejected": 0, "incomplete": 0}
    if expected in ("checked", "built"):
        return rank.get(observed, -1) >= rank["checked"]
    return observed == expected


def main() -> int:
    witness = sys.argv[1]
    cfg = json.loads(WITNESS_CFG.read_text())
    if cfg["witness"] != witness:
        return 125
    text = REC.read_text() if REC.exists() else ""
    if infrastructure_skip(text):
        return 125
    recs = parse_records(text)
    if witness not in recs:
        return 125
    obs = recs[witness]
    status = obs["status"]
    diag = (obs["diagnostics"] or [""])[0]
    diag_norm = norm_diag(diag) if diag else ""

    if status_satisfies_expected(status, cfg["expected_good_status"]):
        return 0

    pin_status = cfg["pin_status"]
    pin_needle = cfg.get("pin_first_diagnostic", "")[:96]
    if status == pin_status:
        if pin_needle and pin_needle in diag:
            return 1
        if diag_norm == cfg["pin_diag_norm"]:
            return 1

    return 125


if __name__ == "__main__":
    raise SystemExit(main())
