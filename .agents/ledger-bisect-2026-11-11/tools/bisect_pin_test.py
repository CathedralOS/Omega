#!/usr/bin/env python3
"""Bisect against fixed PIN ledger + witness PIN observation."""
from __future__ import annotations

import json
import sys
from pathlib import Path

from classify_corpus import norm_diag, parse_records

SNAP_DIR = Path("/tmp/ledger-bisect-pin")
LEDGER = SNAP_DIR / "corpus_outcomes.txt"


def ledger_expected_status(fixture: str) -> tuple[str, str]:
    for line in LEDGER.read_text().splitlines():
        if not line.startswith(fixture + " "):
            continue
        parts = line.split()
        status = parts[1]
        tier = fixture.split("/")[0]
        for p in parts[3:]:
            if p.startswith("regressed-from:"):
                return p.split(":", 1)[1], tier
        return status, tier
    raise KeyError(fixture)


def rank_status(tier: str, status: str) -> int:
    if tier == "fail":
        return 2 if status in ("rejected", "incomplete") else 1
    return {"built": 2, "checked": 1}.get(status, 0)


def load_witness_snapshot(witness: str) -> dict:
    return json.loads((SNAP_DIR / f"witness_{witness.replace('/', '__')}.json").read_text())


def main() -> int:
    witness = sys.argv[1]
    rec_path = Path(sys.argv[2])
    snap = load_witness_snapshot(witness)
    observed = parse_records(rec_path.read_text()) if rec_path.exists() else {}
    if witness not in observed:
        return 125
    o = observed[witness]
    obs_status = o["status"]
    obs_diag = (o["diagnostics"] or [""])[0]
    expected, tier = ledger_expected_status(witness)
    pin_status = snap["pin_status"]
    pin_norm = snap["pin_diag_norm"]

    if rank_status(tier, obs_status) >= rank_status(tier, expected):
        return 0
    if obs_status == pin_status and norm_diag(obs_diag) == pin_norm:
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
