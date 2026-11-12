#!/usr/bin/env python3
"""Classify one corpus record against the fixed PIN ledger and PIN diagnostic.

Exit 0: outcome matches the PIN ledger expectation.
Exit 1: PIN broken status and the PIN first-diagnostic text, exactly.
Exit 125: any other completed outcome, a build failure, or a ledger-only commit.

BISECT_MODE=stops treats every completed non-good outcome as exit 1. That mode
is only for locating the commit where the row stops matching the ledger. The
PIN-diagnostic bisect uses the default mode.
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path


def parse_record(text: str, witness: str) -> tuple[str, str] | None:
    status = None
    diag = ""
    for line in text.splitlines():
        if line.startswith(witness + " "):
            parts = line.split()
            if len(parts) >= 2:
                status = parts[1]
            diag = ""
            continue
        if line.startswith("\t") and status is not None and not diag:
            diag = line[1:].strip()
            continue
        if status is not None and line and not line.startswith("\t"):
            break
    if status is None:
        return None
    return status, diag


def ledger_expected(path: Path, witness: str) -> str | None:
    for line in path.read_text().splitlines():
        if not line.startswith(witness + " "):
            continue
        parts = line.split()
        expected = parts[1]
        for tok in parts[3:]:
            if tok.startswith("regressed-from:"):
                expected = tok.split(":", 1)[1]
        return expected
    return None


def rank(status: str) -> int:
    return {"built": 2, "checked": 1, "rejected": 0, "incomplete": 0}.get(status, -1)


def is_good(witness: str, status: str, expected: str) -> bool:
    tier = witness.split("/")[0]
    if tier == "fail":
        return status == expected
    if status == expected:
        return True
    return expected in ("checked", "built") and status in ("checked", "built") and rank(status) >= rank(expected)


def classify(witness: str, status: str | None, diag: str, expected: str | None, pin_status: str, pin_diag: str, mode: str) -> int:
    if status is None or expected is None:
        return 125
    good = is_good(witness, status, expected)
    pin_bad = status == pin_status and diag == pin_diag
    if mode == "stops":
        if good:
            return 0
        return 1
    if pin_bad:
        return 1
    if good:
        return 0
    return 125


def main() -> int:
    mode = os.environ.get("BISECT_MODE", "pin")
    state = json.loads(Path(os.environ["BISECT_PIN_STATE"]).read_text())
    ledger_path = Path(os.environ["BISECT_PIN_LEDGER"])
    if len(sys.argv) >= 3 and sys.argv[1] == "--from-obs":
        obs = json.loads(Path(sys.argv[2]).read_text())
        witness = os.environ["BISECT_WITNESS"]
        if obs.get("ledger_only") or obs.get("error"):
            return 125
        info = state["fixtures"][witness]
        expected = ledger_expected(ledger_path, witness)
        return classify(
            witness,
            obs.get("status"),
            obs.get("diag") or "",
            expected,
            info["pin_status"],
            info["pin_diag"],
            mode,
        )
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    info = state["fixtures"][witness]
    parsed = parse_record(rec_path.read_text() if rec_path.exists() else "", witness)
    status, diag = parsed if parsed else (None, "")
    expected = ledger_expected(ledger_path, witness)
    return classify(witness, status, diag, expected, info["pin_status"], info["pin_diag"], mode)


if __name__ == "__main__":
    raise SystemExit(main())
