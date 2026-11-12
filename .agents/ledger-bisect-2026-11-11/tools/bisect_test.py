#!/usr/bin/env python3
"""Classify one corpus_runner record against fixed PIN ledger + PIN witness observation."""
from __future__ import annotations

import json
import sys
from pathlib import Path

PIN_CFG = Path("/tmp/ledger_bisect_witness.json")
sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag  # noqa: E402


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1]


def first_diagnostic(text: str, fixture: str) -> str:
    current: str | None = None
    for line in text.splitlines():
        if line.startswith("\t") and current == fixture:
            return line[1:].strip()
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) < 3:
            continue
        current = parts[0]
    return ""


def rank(status: str) -> int:
    return {"built": 3, "checked": 2, "rejected": 1, "incomplete": 0}.get(status, -1)


def meets_expectation(expected: str, observed: str, tier: str) -> bool:
    if observed == expected:
        return True
    if tier == "fail":
        return expected in ("rejected", "incomplete") and observed in (
            "rejected",
            "incomplete",
        )
    return rank(observed) >= rank(expected) and rank(observed) >= 0


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    if not PIN_CFG.exists():
        return 125
    cfg = json.loads(PIN_CFG.read_text())
    text = rec_path.read_text() if rec_path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return 125
    fixture, status = parse_header(line)
    tier = fixture.split("/")[0]
    diag = first_diagnostic(text, fixture)
    diag_norm = norm_diag(diag)
    pin_status = cfg["pin_broken_status"]
    pin_norm = cfg["pin_diag_norm"]
    expected = cfg["expected_status"]
    if status == pin_status:
        return 1
    if meets_expectation(expected, status, tier):
        return 0
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
