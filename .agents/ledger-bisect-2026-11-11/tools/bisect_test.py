#!/usr/bin/env python3
"""Bisect: compare witness run to PIN-fixed ledger expectation only."""
from __future__ import annotations

import sys
from pathlib import Path

LEDGER = Path("/tmp/ledger_bisect_corpus_outcomes.txt")


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1]


def expected_status(ledger_line: str) -> str:
    parts = ledger_line.split()
    if len(parts) < 2:
        raise ValueError("bad ledger line")
    for token in parts[3:]:
        if token.startswith("regressed-from:"):
            return token.split(":", 1)[1]
    return parts[1]


def outcome_ok(tier: str, expected: str, observed: str) -> bool:
    if tier == "fail":
        return observed == expected
    rank = {"built": 2, "checked": 1, "rejected": 0, "incomplete": 0, "timeout": 0}
    return rank.get(observed, -1) >= rank.get(expected, 1)


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    if not LEDGER.is_file():
        return 125
    ledger_line = next(
        (l for l in LEDGER.read_text().splitlines() if l.startswith(witness + " ")),
        None,
    )
    if not ledger_line:
        return 125
    tier = witness.split("/")[0]
    expected = expected_status(ledger_line)
    text = rec_path.read_text() if rec_path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return 125
    _, observed = parse_header(line)
    return 0 if outcome_ok(tier, expected, observed) else 1


if __name__ == "__main__":
    raise SystemExit(main())
