#!/usr/bin/env python3
"""Copy PIN ledger + witness pin observation to /tmp for bisect."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402

REPO = Path(os.environ.get("OMEGA_REPO", str(Path(__file__).resolve().parents[3])))
TMP_LEDGER = Path("/tmp/ledger-bisect-corpus_outcomes.txt")
TMP_WITNESS = Path("/tmp/ledger-bisect-witness.json")
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
PIN_OBS = Path(
    os.environ.get("PIN_OBS", str(REPO / ".agents/ledger-bisect-2026-11-11/pin_full.txt"))
)


def ledger_expected_status(line: str) -> str:
    parts = line.split()
    for tok in parts:
        if tok.startswith("regressed-from:"):
            return tok.split(":", 1)[1]
    return parts[1]


def main() -> int:
    witness = sys.argv[1]
    ledger_text = subprocess.check_output(
        ["git", "-C", str(REPO), "show", f"{PIN}:tests/omega/corpus_outcomes.txt"],
        text=True,
    )
    TMP_LEDGER.write_text(ledger_text)
    obs = parse_records(PIN_OBS.read_text())
    if witness not in obs:
        print(f"missing PIN observation for {witness}", file=sys.stderr)
        return 1
    o = obs[witness]
    ledger_line = next(
        (ln for ln in TMP_LEDGER.read_text().splitlines() if ln.startswith(witness + " ")),
        None,
    )
    if not ledger_line:
        print(f"missing ledger row for {witness}", file=sys.stderr)
        return 1
    pin_diag = (o["diagnostics"] or [""])[0]
    payload = {
        "fixture": witness,
        "tier": o["tier"],
        "pin_commit": PIN,
        "pin_status": o["status"],
        "pin_diag": pin_diag,
        "pin_diag_norm": norm_diag(pin_diag),
        "ledger_line": ledger_line,
        "ledger_expected_status": ledger_expected_status(ledger_line),
    }
    TMP_WITNESS.write_text(json.dumps(payload, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
