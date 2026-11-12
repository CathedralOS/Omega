#!/usr/bin/env python3
"""Bisect step: 1 if PIN status+diagnostic, 0 otherwise, 125 if no record."""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bisect_test import matches_pin_regression  # noqa: E402
from classify_corpus import parse_records  # noqa: E402

REC = Path("/tmp/bisect_one_record.txt")
META = Path("/tmp/ledger-bisect-witness.json")


def main() -> int:
    if not META.exists() or not REC.exists() or not REC.stat().st_size:
        return 125
    meta = json.loads(META.read_text())
    witness = meta["fixture"]
    records = parse_records(REC.read_text())
    if witness not in records:
        return 125
    obs = records[witness]
    if matches_pin_regression(obs, meta):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
