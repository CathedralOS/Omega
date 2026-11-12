#!/usr/bin/env python3
"""Compare corpus_runner output against fixed PIN ledger and PIN observation."""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

FIXTURE = os.environ.get("BISECT_WITNESS", "")
META = Path(os.environ.get("BISECT_PIN_META", "/tmp/ledger-bisect-pin-meta.json"))
RECORD = Path(os.environ.get("BISECT_RECORD", "/tmp/bisect_one_record.txt"))
LEDGER = Path(os.environ.get("BISECT_PIN_LEDGER", "/tmp/ledger-bisect-pin-corpus_outcomes.txt"))


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_record(text: str, fixture: str) -> tuple[str | None, str | None, list[str]]:
    lines = text.splitlines()
    header = next((l for l in lines if l.startswith(fixture + " ")), None)
    if not header:
        return None, None, []
    status = header.split()[1] if len(header.split()) > 1 else None
    diags: list[str] = []
    i = lines.index(header) + 1
    while i < len(lines) and lines[i].startswith("\t"):
        diags.append(lines[i][1:].strip())
        i += 1
    return status, (diags[0] if diags else None), diags


def ledger_expect(fixture: str) -> str | None:
    for line in LEDGER.read_text().splitlines():
        if line.startswith(fixture + " "):
            parts = line.split()
            for j, p in enumerate(parts):
                if p == "regressed-from:" and j + 1 < len(parts):
                    return parts[j + 1]
            return parts[1]
    return None


def main() -> int:
    if not FIXTURE or not META.is_file() or not LEDGER.is_file():
        return 125
    meta = json.loads(META.read_text())
    pin_status = meta["pin_status"]
    pin_diag = meta.get("pin_first_diagnostic")
    pin_diag_norm = meta.get("pin_diag_norm") or (norm_diag(pin_diag) if pin_diag else "")
    expected = meta.get("ledger_expect") or ledger_expect(FIXTURE)
    if expected is None:
        return 125
    text = RECORD.read_text() if RECORD.exists() else ""
    status, first_diag, _ = parse_record(text, FIXTURE)
    if status is None:
        return 125
    if status == "timeout":
        return 125
    if status == expected:
        return 0
    diag_norm = norm_diag(first_diag or "")
    status_match = status == pin_status
    diag_match = (
        not pin_diag_norm
        or not diag_norm
        or diag_norm == pin_diag_norm
        or pin_diag_norm[:80] in (first_diag or "")
        or diag_norm[:80] in (pin_diag or "")
    )
    if status_match and diag_match:
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
