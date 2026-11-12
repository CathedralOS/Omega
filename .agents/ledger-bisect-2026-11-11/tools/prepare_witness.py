#!/usr/bin/env python3
"""Snapshot PIN ledger row and witness observation for bisect."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path


def parse_record(text: str, witness: str) -> tuple[str, str, list[str]] | None:
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if not line.startswith(witness + " "):
            continue
        parts = line.split()
        status = parts[1]
        diags: list[str] = []
        j = i + 1
        while j < len(lines) and lines[j].startswith("\t"):
            diags.append(lines[j][1:].strip())
            j += 1
        return witness, status, diags
    return None


def ledger_row(ledger_text: str, witness: str) -> str | None:
    for line in ledger_text.splitlines():
        if line.startswith(witness + " "):
            return line.strip()
    return None


def expected_status_from_ledger(line: str) -> str:
    parts = line.split()
    for p in parts[3:]:
        if p.startswith("regressed-from:"):
            return p.split(":", 1)[1].split("+")[0]
    return parts[1]


def main() -> int:
    witness = sys.argv[1]
    pin = Path(os.environ["BISECT_PIN_LEDGER"])
    obs = Path(os.environ["BISECT_PIN_OBSERVATION"])
    out = Path(os.environ["BISECT_WITNESS_SPEC"])
    row = ledger_row(pin.read_text(), witness)
    if not row:
        print(f"missing ledger row for {witness}", file=sys.stderr)
        return 1
    rec = parse_record(obs.read_text(), witness)
    if not rec:
        print(f"missing PIN observation for {witness}", file=sys.stderr)
        return 1
    _, pin_status, pin_diags = rec
    spec = {
        "witness": witness,
        "tier": witness.split("/")[0],
        "ledger_row": row,
        "expected_good_status": expected_status_from_ledger(row),
        "pin_bad_status": pin_status,
        "pin_bad_diag": (pin_diags[0] if pin_diags else ""),
    }
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(spec, indent=2) + "\n")
    print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
