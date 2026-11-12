#!/usr/bin/env python3
"""Classify one witness run against fixed PIN ledger + PIN observation."""
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


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 2:
        return None
    return parts[0], parts[1]


def ledger_line(ledger_text: str, fixture: str) -> str | None:
    for line in ledger_text.splitlines():
        if line.startswith(fixture + " "):
            return line
    return None


def expected_status_from_pin_line(line: str) -> str | None:
    flags = line.split()[3:]
    for f in flags:
        if f.startswith("regressed-from:"):
            rest = f[len("regressed-from:") :]
            return rest.split("+")[0]
    return line.split()[1]


def record_diagnostics(text: str, fixture: str) -> list[str]:
    diags: list[str] = []
    in_fixture = False
    for line in text.splitlines():
        if line.startswith(fixture + " "):
            in_fixture = True
            continue
        if in_fixture:
            if line.startswith("\t"):
                diags.append(line[1:].strip())
            elif line.strip() and not line.startswith("#"):
                break
    return diags


def status_rank(status: str) -> int:
    return {"built": 3, "checked": 2, "rejected": 1, "incomplete": 0}.get(status, -1)


def main() -> int:
    witness = sys.argv[1]
    rec_path = Path(sys.argv[2])
    baseline_path = Path(os.environ["BISECT_WITNESS_BASELINE"])
    baseline = json.loads(baseline_path.read_text())
    pin_ledger = Path(os.environ["BISECT_PIN_LEDGER"]).read_text()
    line = ledger_line(pin_ledger, witness)
    if line is None:
        return 125
    expected = baseline.get("expected_status") or expected_status_from_pin_line(line)
    pin_bad_status = baseline["pin_bad_status"]
    pin_bad_diag_norm = baseline["pin_bad_diag_norm"]

    text = rec_path.read_text() if rec_path.exists() else ""
    header = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not header:
        return 125
    _, observed_status = parse_header(header)
    tier = witness.split("/")[0]
    diags = record_diagnostics(text, witness)
    first_diag = diags[0] if diags else ""
    observed_norm = norm_diag(first_diag)

    if tier == "fail":
        return 125

    if status_rank(observed_status) >= status_rank(expected):
        return 0

    if observed_status == pin_bad_status and observed_norm == pin_bad_diag_norm:
        return 1

    return 125


if __name__ == "__main__":
    raise SystemExit(main())
