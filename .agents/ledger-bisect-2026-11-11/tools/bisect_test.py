#!/usr/bin/env python3
"""Classify one corpus run vs fixed PIN ledger + witness observation."""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path


def parse_record(text: str, witness: str) -> tuple[str, list[str]] | None:
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if not line.startswith(witness + " "):
            continue
        status = line.split()[1]
        diags: list[str] = []
        j = i + 1
        while j < len(lines) and lines[j].startswith("\t"):
            diags.append(lines[j][1:].strip())
            j += 1
        return status, diags
    return None


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def status_satisfies_expected(observed: str, expected: str, tier: str) -> bool:
    if observed == expected:
        return True
    if tier != "fail" and expected == "checked" and observed == "built":
        return True
    return False


def same_bad_class(observed_diags: list[str], pin_diag: str) -> bool:
    if not pin_diag:
        return bool(observed_diags)
    if not observed_diags:
        return False
    if observed_diags[0] == pin_diag:
        return True
    return norm_diag(observed_diags[0]) == norm_diag(pin_diag)


def main() -> int:
    witness = sys.argv[1]
    rec_path = Path(sys.argv[2])
    spec_path = Path(os.environ["BISECT_WITNESS_SPEC"])
    spec = json.loads(spec_path.read_text())
    if witness != spec["witness"]:
        return 125
    parsed = parse_record(rec_path.read_text() if rec_path.exists() else "", witness)
    if not parsed:
        return 125
    status, diags = parsed
    if status_satisfies_expected(status, spec["expected_good_status"], spec["tier"]):
        return 0
    if status == spec["pin_bad_status"] and same_bad_class(diags, spec["pin_bad_diag"]):
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
