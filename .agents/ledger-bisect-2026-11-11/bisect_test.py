#!/usr/bin/env python3
"""Compare one corpus_runner record to fixed PIN witness expectations (not checkout ledger)."""
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
    status: str | None = None
    diags: list[str] = []
    in_fixture = False
    for line in text.splitlines():
        if line.startswith(witness + " "):
            parts = line.split()
            if len(parts) >= 2:
                status = parts[1]
            in_fixture = True
            continue
        if in_fixture:
            if line.startswith("\t"):
                diags.append(line[1:].strip())
                continue
            if line and not line.startswith(" "):
                break
    if status is None:
        return None
    return status, diags


def same_class(pin_diag: str, obs_diag: str, pin_prefix: str | None) -> bool:
    if pin_prefix:
        return obs_diag.startswith(pin_prefix) or pin_diag.startswith(pin_prefix)
    if not pin_diag and not obs_diag:
        return True
    if pin_diag and obs_diag:
        return norm_diag(pin_diag) == norm_diag(obs_diag)
    return False


def is_good_status(expected: str, observed: str, tier: str) -> bool:
    if tier == "fail":
        return observed == expected
    if expected in ("checked", "built"):
        return observed in ("checked", "built")
    return observed == expected


def main() -> int:
    spec_path = Path(os.environ["BISECT_WITNESS_SPEC"])
    rec_path = Path(sys.argv[1])
    spec = json.loads(spec_path.read_text())
    witness = spec["witness"]
    text = rec_path.read_text() if rec_path.exists() else ""
    parsed = parse_record(text, witness)
    if not parsed:
        return 125
    status, diags = parsed
    tier = witness.split("/")[0]
    obs_diag = diags[0] if diags else ""
    pin_status = spec["pin_status"]
    pin_diag = spec.get("pin_first_diagnostic", "")
    pin_prefix = spec.get("pin_first_diagnostic_prefix")
    expected = spec["expected_good_status"]

    if is_good_status(expected, status, tier):
        return 0

    if status == pin_status and same_class(pin_diag, obs_diag, pin_prefix):
        return 1

    return 125


if __name__ == "__main__":
    raise SystemExit(main())
