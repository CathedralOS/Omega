#!/usr/bin/env python3
"""Snapshot PIN ledger row + observed outcome for one witness."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402


def ledger_row(ledger_text: str, fixture: str) -> dict | None:
    for line in ledger_text.splitlines():
        if not line.startswith(fixture + " "):
            continue
        parts = line.split()
        status = parts[1]
        flags = [p for p in parts[3:] if not p.endswith("ms") and not p.startswith("@")]
        regressed = None
        for f in flags:
            if f.startswith("regressed-from:"):
                regressed = f.split(":", 1)[1]
        expected = regressed if regressed else status
        return {"ledger_status": status, "expected_status": expected, "flags": flags}
    return None


def main() -> int:
    witness, ledger_path, observed_path, out_path = sys.argv[1:5]
    ledger_text = Path(ledger_path).read_text()
    row = ledger_row(ledger_text, witness)
    if row is None:
        raise SystemExit(f"missing ledger row for {witness}")
    observed = parse_records(Path(observed_path).read_text())
    if witness not in observed:
        raise SystemExit(f"missing observed record for {witness}")
    obs = observed[witness]
    first_diag = (obs.get("diagnostics") or [""])[0]
    payload = {
        "witness": witness,
        "tier": witness.split("/")[0],
        "expected_status": row["expected_status"],
        "ledger_status": row["ledger_status"],
        "pin_status": obs["status"],
        "pin_first_diagnostic": first_diag,
        "pin_diag_norm": norm_diag(first_diag),
    }
    Path(out_path).write_text(json.dumps(payload, indent=2))
    print(out_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
