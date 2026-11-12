#!/usr/bin/env python3
"""Classify one witness run against frozen PIN ledger + PIN observation."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402

BASE = Path("/tmp/ledger-bisect-pin")
LEDGER = Path(os.environ.get("LEDGER_PIN_FILE", str(BASE / "corpus_outcomes.txt")))


def slug(fixture: str) -> str:
    return fixture.replace("/", "__")


def status_rank(status: str) -> int:
    return {"built": 3, "checked": 2, "incomplete": 1, "rejected": 0}.get(status, -1)


def meets_ledger(expected: str, observed: str, tier: str) -> bool:
    if tier == "fail":
        if expected in ("rejected", "incomplete"):
            return observed in ("rejected", "incomplete")
        return observed == expected
    exp_r = status_rank(expected)
    obs_r = status_rank(observed)
    return obs_r >= exp_r and exp_r >= 0


def reproduces_pin(pin_status: str, pin_norm: str, observed: str, diag: str) -> bool:
    if observed != pin_status:
        return False
    if not pin_norm:
        return True
    dn = norm_diag(diag)
    return dn == pin_norm or pin_norm[:40] in dn or dn[:40] in pin_norm


def main() -> int:
    witness = sys.argv[1]
    rec_path = Path(sys.argv[2])
    meta_path = BASE / "witnesses" / f"{slug(witness)}.json"
    if not meta_path.exists():
        return 125
    meta = json.loads(meta_path.read_text())
    text = rec_path.read_text() if rec_path.exists() else ""
    if not any(ln.startswith(witness + " ") for ln in text.splitlines()):
        return 125
    records = parse_records(text)
    if witness not in records:
        return 125
    obs = records[witness]
    status = obs["status"]
    if status == "timeout":
        return 125
    diag = (obs["diagnostics"] or [""])[0]
    if meets_ledger(meta["expected_status"], status, meta["tier"]):
        return 0
    if reproduces_pin(meta["pin_status"], meta["pin_diag_norm"], status, diag):
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
