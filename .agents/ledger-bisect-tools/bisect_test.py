#!/usr/bin/env python3
"""Bisect step: compare witness run to fixed PIN ledger and PIN observation."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

PIN_OBS = Path("/tmp/ledger_bisect_witness_pin.json")
PIN_LEDGER = Path("/tmp/ledger_bisect_corpus_outcomes.txt")


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1]


def parse_ledger_line(fixture: str, text: str) -> dict | None:
    for line in text.splitlines():
        if line.startswith(fixture + " "):
            parts = line.split()
            status = parts[1]
            regressed_from = None
            for tok in parts[3:]:
                if tok.startswith("regressed-from:"):
                    regressed_from = tok.split(":", 1)[1]
            expected = regressed_from if regressed_from else status
            return {"ledger_status": status, "expected_status": expected}
    return None


def first_diagnostic(text: str, fixture: str) -> str:
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if line.startswith(fixture + " "):
            for j in range(i + 1, len(lines)):
                if lines[j].startswith("\t"):
                    return lines[j].lstrip("\t").strip()
                if lines[j].strip() and not lines[j].startswith("\t"):
                    break
    return ""


def status_rank(status: str) -> int:
    return {"built": 3, "checked": 2, "rejected": 1, "incomplete": 0, "timeout": -1}.get(status, -1)


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    if not PIN_OBS.is_file() or not PIN_LEDGER.is_file():
        return 125
    pin = json.loads(PIN_OBS.read_text())
    if pin["witness"] != witness:
        return 125
    ledger = parse_ledger_line(witness, PIN_LEDGER.read_text())
    if not ledger:
        return 125
    expected = pin.get("expected_status") or ledger["expected_status"]

    text = rec_path.read_text() if rec_path.exists() else ""
    header = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not header:
        return 125
    _, observed = parse_header(header) or ("", "")
    if not observed:
        return 125

    if status_rank(observed) >= status_rank(expected):
        return 0

    tier = witness.split("/")[0]
    obs_diag = first_diagnostic(text, witness)
    pin_diag = pin.get("first_diagnostic", "")

    if tier in ("pass", "run") and status_rank(observed) < status_rank(expected):
        if observed == pin["pin_status"] and (
            not pin_diag
            or obs_diag == pin_diag
            or norm_diag(obs_diag) == norm_diag(pin_diag)
        ):
            return 1
        if observed == "rejected" and pin["pin_status"] == "rejected":
            return 1

    if tier == "fail" and pin["pin_status"] not in ("rejected", "incomplete"):
        if observed == pin["pin_status"]:
            return 1

    return 125


if __name__ == "__main__":
    raise SystemExit(main())
