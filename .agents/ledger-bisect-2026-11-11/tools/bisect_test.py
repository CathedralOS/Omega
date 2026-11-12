#!/usr/bin/env python3
"""Bisect test using fixed PIN ledger + PIN witness observation (not checkout ledger)."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402

RANK = {"incomplete": 0, "rejected": 0, "checked": 1, "built": 2, "timeout": -1}


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1]


def meets_expected(tier: str, expected: str, status: str, expected_satisfied: bool | None) -> bool:
    if tier == "fail":
        return status in ("rejected", "incomplete")
    if RANK.get(status, -1) >= RANK.get(expected, -1):
        if expected_satisfied is False:
            return False
        return True
    return False


def matches_pin_broken(meta: dict, status: str, first_diag: str) -> bool:
    if status != meta["pin_status"]:
        return False
    if not first_diag and not meta["pin_first_diagnostic"]:
        return True
    if norm_diag(first_diag) == meta["pin_diag_norm"]:
        return True
    # Same class: status matches and diagnostic shares normalized prefix
    a, b = norm_diag(first_diag), meta["pin_diag_norm"]
    n = min(len(a), len(b), 96)
    return a[:n] == b[:n] and n >= 48


def main() -> int:
    witness = os.environ.get("BISECT_WITNESS")
    meta_path = os.environ.get("BISECT_WITNESS_PIN")
    rec_path = Path(sys.argv[1] if len(sys.argv) > 1 else "/tmp/bisect_one_record.txt")
    if not witness or not meta_path:
        return 125
    meta = json.loads(Path(meta_path).read_text())
    text = rec_path.read_text() if rec_path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return 125
    _, status = parse_header(line) or (None, None)
    if status is None:
        return 125
    records = parse_records(text)
    obs = records.get(witness)
    if not obs:
        return 125
    first_diag = (obs.get("diagnostics") or [""])[0]
    tier = meta["tier"]
    if meets_expected(tier, meta["expected_status"], status, obs.get("expected_satisfied")):
        return 0
    # Any failure to meet the PIN ledger expectation is bad for bisect; PIN signature match
    # is the witness confirmation at the ends, not at every intermediate diagnostic.
    if matches_pin_broken(meta, status, first_diag):
        return 1
    if tier == "fail":
        if status not in ("rejected", "incomplete"):
            return 1
    elif RANK.get(status, -1) < RANK.get(meta["expected_status"], -1):
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
