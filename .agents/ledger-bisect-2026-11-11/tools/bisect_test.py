#!/usr/bin/env python3
"""Score one corpus record against the frozen PIN observation.

Predicates (BISECT_PREDICATE):
  outcome — 0 ledger-expected status, 1 PIN broken status, 125 otherwise.
  class   — 1 PIN diagnostic class, 0 any other parsed outcome, 125 no record.
"""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

OBS_PATH = Path("/tmp/pin_witness_observation.json")


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_records(text: str) -> dict[str, tuple[str, list[str]]]:
    found: dict[str, tuple[str, list[str]]] = {}
    cur = None
    status = ""
    diags: list[str] = []

    def finish() -> None:
        nonlocal cur, status, diags
        if cur is not None:
            found[cur] = (status, diags)

    for line in text.splitlines():
        if line.startswith("\t"):
            if cur is not None:
                diags.append(line[1:].strip())
            continue
        parts = line.split()
        if len(parts) >= 3 and parts[2].endswith("ms"):
            finish()
            cur = parts[0]
            status = parts[1]
            diags = []
    finish()
    return found


def same_class(diag_norm: str, pin_norm: str) -> bool:
    if not pin_norm:
        return False
    if diag_norm == pin_norm:
        return True
    n = min(len(diag_norm), len(pin_norm), 80)
    return n > 0 and diag_norm[:n] == pin_norm[:n]


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    predicate = os.environ.get("BISECT_PREDICATE", "outcome")
    detail = Path("/tmp/bisect_last_detail.json")
    if not OBS_PATH.exists() or not rec_path.exists():
        detail.write_text(json.dumps({"exit": 125, "reason": "missing record or observation"}) + "\n")
        return 125
    obs = json.loads(OBS_PATH.read_text())
    parsed = parse_records(rec_path.read_text())
    if witness not in parsed:
        detail.write_text(
            json.dumps({"exit": 125, "reason": "witness absent", "predicate": predicate, "witness": witness})
            + "\n"
        )
        return 125
    status, diags = parsed[witness]
    first = diags[0] if diags else ""
    dn = norm_diag(first)
    expected = obs["expected_good_status"]
    pin_status = obs["pin_bad_status"]
    pin_norm = obs.get("pin_bad_diag_norm") or ""
    pin_diag = obs.get("pin_bad_diag") or ""

    if predicate == "class":
        code = 1 if status == pin_status and same_class(dn, pin_norm) else 0
    elif predicate == "outcome":
        if status == expected:
            code = 0
        elif status == pin_status:
            code = 1
        else:
            code = 125
    else:
        code = 125

    payload = {
        "exit": code,
        "predicate": predicate,
        "witness": witness,
        "status": status,
        "diag": first,
        "diag_norm": dn,
        "pin_diag": pin_diag,
        "diag_equals_pin": first == pin_diag,
        "diag_same_class": same_class(dn, pin_norm),
        "members": {
            name: {"status": st, "diag": (ds[0] if ds else "")} for name, (st, ds) in sorted(parsed.items())
        },
    }
    detail.write_text(json.dumps(payload) + "\n")
    return code


if __name__ == "__main__":
    raise SystemExit(main())
