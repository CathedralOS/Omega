#!/usr/bin/env python3
"""Bisect step test using fixed PIN ledger + witness observation in /tmp."""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402

TMP_WITNESS = Path("/tmp/ledger-bisect-witness.json")
REC = Path("/tmp/bisect_one_record.txt")

PASS_OK = {"built", "checked"}
FAIL_OK = {"rejected", "incomplete"}


def status_rank(status: str) -> int:
    order = {"built": 3, "checked": 2, "rejected": 1, "incomplete": 1, "timeout": 0}
    return order.get(status, 0)


def meets_ledger_expected(obs_status: str, meta: dict) -> bool:
    tier = meta["tier"]
    want = meta["ledger_expected_status"]
    if tier == "fail":
        return obs_status in FAIL_OK and want in FAIL_OK
    if want in ("checked", "built"):
        return obs_status in PASS_OK and status_rank(obs_status) >= status_rank(want)
    return obs_status == want


def matches_pin_regression(obs: dict, meta: dict) -> bool:
    if not meets_ledger_expected(obs["status"], meta):
        obs_diag = norm_diag((obs.get("diagnostics") or [""])[0])
        if obs["status"] == meta["pin_status"] and obs_diag == meta["pin_diag_norm"]:
            return True
        if meta["tier"] != "fail" and obs["status"] not in PASS_OK:
            return True
        if meta["tier"] == "fail" and obs["status"] not in FAIL_OK:
            return True
    return False


def main() -> int:
    if not TMP_WITNESS.exists():
        return 125
    meta = json.loads(TMP_WITNESS.read_text())
    witness = meta["fixture"]
    if not REC.exists() or not REC.stat().st_size:
        return 125
    records = parse_records(REC.read_text())
    if witness not in records:
        return 125
    obs = records[witness]
    if meets_ledger_expected(obs["status"], meta):
        return 0
    if matches_pin_regression(obs, meta):
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
