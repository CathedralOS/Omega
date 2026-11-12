#!/usr/bin/env python3
"""Snapshot PIN ledger + witness observation for bisect."""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
OUT_DIR = Path("/tmp/ledger-bisect-pin")


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_pin_record(records_text: str, witness: str) -> tuple[str, str]:
    status = None
    diag = ""
    for line in records_text.splitlines():
        if line.startswith(witness + " "):
            status = line.split()[1]
            continue
        if line.startswith("\t") and status is not None:
            diag = line[1:].strip()
            break
    if status is None:
        raise SystemExit(f"witness {witness} missing from pin_records")
    return status, diag


def main() -> int:
    repo = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
    witness = sys.argv[1]
    pin_records = repo / ".agents/ledger-bisect-2026-11-11/pin_records.txt"
    if not pin_records.is_file():
        raise SystemExit("missing pin_records.txt")
    ledger_text = subprocess.check_output(
        ["git", "show", f"{PIN}:tests/omega/corpus_outcomes.txt"], text=True
    )
    pin_status, pin_diag = parse_pin_record(pin_records.read_text(), witness)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    ledger_path = OUT_DIR / "corpus_outcomes.txt"
    ledger_path.write_text(ledger_text)
    state = {
        "witness": witness,
        "pin": PIN,
        "pin_status": pin_status,
        "pin_diag": pin_diag,
        "pin_diag_norm": norm_diag(pin_diag),
    }
    (OUT_DIR / "witness_state.json").write_text(json.dumps(state, indent=2))
    print(str(ledger_path))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
