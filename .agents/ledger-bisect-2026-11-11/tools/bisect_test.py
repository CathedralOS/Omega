#!/usr/bin/env python3
"""Bisect test vs fixed PIN ledger and PIN witness observation (not checkout ledger)."""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_record(text: str, witness: str) -> tuple[str, list[str]] | None:
    status = None
    diags: list[str] = []
    for line in text.splitlines():
        if line.startswith(witness + " "):
            parts = line.split()
            if len(parts) >= 2:
                status = parts[1]
            continue
        if line.startswith("\t") and status is not None:
            diags.append(line[1:].strip())
            continue
        if status is not None and not line.startswith("\t"):
            break
    if status is None:
        return None
    return status, diags


def ledger_row(path: Path, witness: str) -> dict | None:
    for line in path.read_text().splitlines():
        if not line.startswith(witness + " "):
            continue
        parts = line.split()
        status = parts[1]
        regressed_from = None
        for tok in parts[3:]:
            if tok.startswith("regressed-from:"):
                regressed_from = tok.split(":", 1)[1]
            if tok in ("expected", "unexpected"):
                pass
        return {"status": status, "regressed_from": regressed_from}
    return None


def rank(status: str) -> int:
    return {"built": 2, "checked": 1, "rejected": 0, "incomplete": 0}.get(status, -1)


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    state_path = Path(os.environ["BISECT_PIN_STATE"])
    ledger_path = Path(os.environ["BISECT_PIN_LEDGER"])
    state = json.loads(state_path.read_text())
    if witness != state["witness"]:
        return 125
    text = rec_path.read_text() if rec_path.exists() else ""
    parsed = parse_record(text, witness)
    if not parsed:
        return 125
    obs_status, diags = parsed
    obs_diag = diags[0] if diags else ""

    ledger = ledger_row(ledger_path, witness)
    if ledger is None:
        return 125

    tier = witness.split("/")[0]
    expected = ledger["status"]
    if ledger.get("regressed_from"):
        expected = ledger["regressed_from"]

    pin_status = state["pin_status"]
    pin_diag = state.get("pin_diag", "")
    pin_norm = state.get("pin_diag_norm") or norm_diag(pin_diag)
    obs_norm = norm_diag(obs_diag)

    def pin_bad() -> bool:
        if obs_status != pin_status:
            return False
        if pin_diag and obs_diag.startswith(pin_diag[: min(80, len(pin_diag))]):
            return True
        return obs_norm == pin_norm

    def ledger_good() -> bool:
        if tier == "fail":
            return obs_status in ("rejected", "incomplete")
        if obs_status == expected:
            return True
        if expected in ("checked", "built") and rank(obs_status) >= rank(expected):
            return True
        return False

    if pin_bad():
        return 1
    if ledger_good():
        return 0
    if tier != "fail":
        if obs_status in ("rejected", "incomplete") or rank(obs_status) < rank(expected):
            return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
