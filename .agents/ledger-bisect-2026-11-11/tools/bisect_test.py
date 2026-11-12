#!/usr/bin/env python3
"""Classify one corpus_runner record vs fixed PIN ledger + PIN bad observation."""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

def _pin_ledger() -> Path:
    return Path(os.environ["BISECT_PIN_LEDGER"])


def _profile_path() -> Path:
    return Path(os.environ["BISECT_WITNESS_PROFILE"])


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_record(text: str, fixture: str) -> dict | None:
    lines = text.splitlines()
    header = next((l for l in lines if l.startswith(fixture + " ")), None)
    if not header:
        return None
    parts = header.split()
    status = parts[1]
    tier = fixture.split("/")[0]
    diags = []
    for line in lines:
        if line.startswith("\t"):
            diags.append(line[1:].strip())
        elif line.startswith(fixture + " "):
            continue
        elif diags and line.strip():
            break
    return {"fixture": fixture, "tier": tier, "status": status, "diagnostics": diags}


def ledger_expected(fixture: str, ledger_text: str) -> str | None:
    for line in ledger_text.splitlines():
        if not line.startswith(fixture + " "):
            continue
        parts = line.split()
        status = parts[1]
        for p in parts[3:]:
            if p.startswith("regressed-from:"):
                return p.split(":", 1)[1]
        tier = fixture.split("/")[0]
        if tier == "fail":
            return "rejected"
        return status
    return None


def matches_expected(obs: dict, expected: str) -> bool:
    tier = obs["tier"]
    status = obs["status"]
    if tier == "fail":
        return status in ("rejected", "incomplete")
    rank = {"built": 2, "checked": 1, "rejected": 0, "incomplete": 0, "timeout": 0}

    def r(s: str) -> int:
        return rank.get(s, -1)

    return r(status) >= r(expected)


def matches_pin_bad(obs: dict, profile: dict, expected: str) -> bool:
    if matches_expected(obs, expected):
        return False
    tier = obs["tier"]
    status = obs["status"]
    if tier == "fail":
        return status not in ("rejected", "incomplete")
    # pass/run: any regression from ledger expectation is bad for bisect.
    if status in ("rejected", "incomplete", "timeout"):
        if profile.get("pin_diag_norm") and obs["diagnostics"]:
            got = norm_diag(obs["diagnostics"][0])
            if got == profile["pin_diag_norm"]:
                return True
        # Same class: still worse than ledger even if diagnostic text shifted.
        return True
    return False


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    profile = json.loads(_profile_path().read_text())
    ledger_text = _pin_ledger().read_text()
    expected = ledger_expected(witness, ledger_text)
    if expected is None:
        return 125
    text = rec_path.read_text() if rec_path.exists() else ""
    obs = parse_record(text, witness)
    if obs is None:
        return 125
    if matches_expected(obs, expected):
        return 0
    if matches_pin_bad(obs, profile, expected):
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
