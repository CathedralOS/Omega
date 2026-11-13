#!/usr/bin/env python3
"""Classify one corpus record for a bisect witness.

BISECT_MODE=stop: 0 when the frozen PIN ledger outcome is met, 1 for any other outcome.
BISECT_MODE=class: 1 when the first diagnostic is the PIN class, 0 otherwise.
Exit 125 only when the record is missing, unreadable, or timed out (the runner did not finish).
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402

BASE = Path("/tmp/ledger-bisect-pin")


def slug(fixture: str) -> str:
    return fixture.replace("/", "__")


def status_rank(status: str) -> int:
    return {"built": 3, "checked": 2, "incomplete": 1, "rejected": 0}.get(status, -1)


def meets_ledger(expected: str, observed: str, tier: str) -> bool:
    if tier == "fail":
        if expected in ("rejected", "incomplete"):
            return observed in ("rejected", "incomplete")
        return observed == expected
    return status_rank(observed) >= status_rank(expected) >= 0


def same_class(pin_norm: str, diag: str) -> bool:
    dn = norm_diag(diag)
    if not pin_norm or not dn:
        return False
    if dn == pin_norm:
        return True
    stem = pin_norm[:48]
    other = dn[:48]
    if len(stem) < 24 or len(other) < 24:
        return False
    return stem in dn or other in pin_norm


def main() -> int:
    witness = sys.argv[1]
    rec_path = Path(sys.argv[2])
    mode = os.environ.get("BISECT_MODE", "stop")
    meta_path = BASE / "witnesses" / f"{slug(witness)}.json"
    if not meta_path.exists() or not rec_path.exists():
        return 125
    meta = json.loads(meta_path.read_text())
    text = rec_path.read_text()
    if not any(ln.startswith(witness + " ") for ln in text.splitlines()):
        return 125
    records = parse_records(text)
    obs = records.get(witness)
    if obs is None or obs["status"] == "timeout":
        return 125
    diag = (obs["diagnostics"] or [""])[0]
    if mode == "class":
        return 1 if same_class(meta["pin_diag_norm"], diag) else 0
    if mode == "stop":
        return 0 if meets_ledger(meta["expected_status"], obs["status"], meta["tier"]) else 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
