#!/usr/bin/env python3
"""Label each fixture in a corpus record file vs fixed PIN ledger + pin_full."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bisect_test import meets_ledger_expected, matches_pin_regression  # noqa: E402
from classify_corpus import norm_diag, parse_records  # noqa: E402

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
import os

REPO = Path(os.environ.get("OMEGA_REPO", str(Path(__file__).resolve().parents[3])))


def ledger_expected_status(line: str) -> str:
    for tok in line.split():
        if tok.startswith("regressed-from:"):
            return tok.split(":", 1)[1]
    return line.split()[1]


def meta_for(fixture: str, ledger_text: str, pin_records: dict) -> dict | None:
    line = next((ln for ln in ledger_text.splitlines() if ln.startswith(fixture + " ")), None)
    if not line:
        return None
    pin = pin_records.get(fixture)
    if not pin:
        return None
    return {
        "fixture": fixture,
        "tier": fixture.split("/")[0],
        "ledger_expected_status": ledger_expected_status(line),
        "pin_status": pin["status"],
        "pin_diag_norm": norm_diag((pin["diagnostics"] or [""])[0]),
    }


def classify(meta: dict, obs: dict) -> str:
    if meets_ledger_expected(obs["status"], meta):
        return "good"
    if matches_pin_regression(obs, meta):
        return "bad"
    return "ambiguous"


def main() -> int:
    rec_path = Path(sys.argv[1])
    members = sys.argv[2:]
    import subprocess

    ledger_text = subprocess.check_output(
        ["git", "show", f"{PIN}:tests/omega/corpus_outcomes.txt"],
        cwd=REPO,
        text=True,
    )
    import os

    pin_path = Path(
        os.environ.get("PIN_OBS", str(REPO / ".agents/ledger-bisect-2026-11-11/pin_full.txt"))
    )
    pin_records = parse_records(pin_path.read_text())
    observed = parse_records(rec_path.read_text())
    for m in members:
        meta = meta_for(m, ledger_text, pin_records)
        if not meta or m not in observed:
            print(f"{m}\tskip\t125")
            continue
        label = classify(meta, observed[m])
        code = {"good": 0, "bad": 1, "ambiguous": 125}[label]
        print(f"{m}\t{label}\t{code}\t{observed[m]['status']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
