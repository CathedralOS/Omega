#!/usr/bin/env python3
"""Write fixed PIN expectations for one witness (ledger line + broken observation)."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag  # noqa: E402


def parse_fixture_record(text: str, fixture: str) -> dict | None:
    current: str | None = None
    records: dict[str, dict] = {}
    for line in text.splitlines():
        if line.startswith("\t") and current:
            records[current].setdefault("diagnostics", []).append(line[1:].strip())
            continue
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) < 3:
            continue
        fix, status = parts[0], parts[1]
        flags = [p for p in parts[3:] if not p.startswith("@") and not p.endswith("ms")]
        regressed_from = None
        for f in flags:
            if f.startswith("regressed-from:"):
                regressed_from = f.split(":", 1)[1]
        records[fix] = {
            "fixture": fix,
            "status": status,
            "regressed_from": regressed_from,
            "diagnostics": [],
        }
        current = fix
    return records.get(fixture)


def ledger_expectation(ledger: dict) -> str:
    if ledger.get("regressed_from"):
        return ledger["regressed_from"]
    return ledger["status"]


def main() -> int:
    witness, ledger_path, observed_path = sys.argv[1:4]
    ledger = parse_fixture_record(Path(ledger_path).read_text(), witness)
    observed = parse_fixture_record(Path(observed_path).read_text(), witness)
    if not ledger:
        raise SystemExit(f"missing {witness} in pin ledger")
    if not observed:
        raise SystemExit(f"missing {witness} in pin observation")
    pin_diag = (observed.get("diagnostics") or [""])[0]
    out = {
        "witness": witness,
        "expected_status": ledger_expectation(ledger),
        "pin_broken_status": observed["status"],
        "pin_first_diagnostic": pin_diag,
        "pin_diag_norm": norm_diag(pin_diag),
    }
    Path("/tmp/ledger_bisect_witness.json").write_text(json.dumps(out, indent=2) + "\n")
    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
