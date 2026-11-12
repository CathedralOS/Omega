#!/usr/bin/env python3
"""Compare one witness run against frozen PIN ledger expectation and PIN breakage."""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402

META_PATH = Path("/tmp/ledger_bisect_witness_meta.json")


def load_meta() -> dict:
    return json.loads(META_PATH.read_text())


def first_diagnostic(records: dict, witness: str) -> str:
    if witness not in records:
        return ""
    return (records[witness].get("diagnostics") or [""])[0]


def same_diagnostic_class(pin_norm: str, observed_norm: str) -> bool:
    if not pin_norm:
        return not observed_norm
    return pin_norm == observed_norm


def main() -> int:
    if not META_PATH.is_file():
        return 125
    meta = load_meta()
    witness, rec_path = meta["witness"], Path(sys.argv[1])
    text = rec_path.read_text() if rec_path.exists() else ""
    records = parse_records(text)
    if witness not in records:
        return 125

    observed = records[witness]
    status = observed["status"]
    expected = meta["expected_status"]
    pin_status = meta["pin_broken_status"]
    tier = meta["tier"]
    diag = first_diagnostic(records, witness)
    diag_norm = norm_diag(diag)

    if status == expected:
        return 0

    if status == pin_status and same_diagnostic_class(meta["pin_diag_norm"], diag_norm):
        return 1

    # Fail fixture may pin expected/unexpected without changing tier status semantics.
    if tier == "fail" and status == pin_status:
        return 1

    return 125


if __name__ == "__main__":
    raise SystemExit(main())
