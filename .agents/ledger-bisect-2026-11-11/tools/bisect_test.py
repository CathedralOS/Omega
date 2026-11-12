#!/usr/bin/env python3
"""Compare one witness run against fixed PIN ledger + PIN witness snapshot."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag  # noqa: E402

LEDGER = Path("/tmp/pin_corpus_outcomes.txt")
SNAPSHOT = Path("/tmp/pin_witness_snapshot.json")


def parse_ledger_row(fixture: str, text: str) -> tuple[str, str | None] | None:
    for line in text.splitlines():
        if not line.startswith(fixture + " "):
            continue
        parts = line.split()
        if len(parts) < 2:
            return None
        status = parts[1]
        regressed_from = None
        for token in parts[2:]:
            if token.startswith("regressed-from:"):
                regressed_from = token.split(":", 1)[1]
        expected = regressed_from if regressed_from else status
        return status, expected
    return None


def parse_record_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1]


def first_diagnostic(text: str, fixture: str) -> str:
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if line.startswith(fixture + " "):
            for j in range(i + 1, len(lines)):
                if lines[j].startswith("\t"):
                    return lines[j][1:].strip()
                if lines[j].strip() and not lines[j].startswith("#"):
                    break
    return ""


def status_ok(tier: str, observed: str, expected: str) -> bool:
    if tier == "fail":
        return observed == expected
    rank = {"built": 3, "checked": 2, "rejected": 1, "incomplete": 0}
    return rank.get(observed, 0) >= rank.get(expected, 2)


def main() -> int:
    witness = sys.argv[1]
    rec_path = Path(sys.argv[2])
    if not LEDGER.is_file() or not SNAPSHOT.is_file():
        return 125
    snap = json.loads(SNAPSHOT.read_text())
    if witness != snap["witness"]:
        return 125
    ledger = LEDGER.read_text()
    row = parse_ledger_row(witness, ledger)
    if row is None:
        return 125
    _ledger_status, expected_status = row
    tier = witness.split("/")[0]

    text = rec_path.read_text() if rec_path.exists() else ""
    header = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not header:
        return 125
    fixture, observed = parse_record_header(header)
    if fixture is None:
        return 125
    diag = first_diagnostic(text, witness)

    pin_status = snap["pin_status"]
    pin_diag_norm = snap["pin_diag_norm"]

    if status_ok(tier, observed, expected_status):
        return 0
    if observed == pin_status and norm_diag(diag) == pin_diag_norm:
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
