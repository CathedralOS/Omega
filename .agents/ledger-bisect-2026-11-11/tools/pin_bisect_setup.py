#!/usr/bin/env python3
"""Freeze PIN ledger + witness observation for bisect comparisons."""
from __future__ import annotations

import json
import re
import shutil
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402
from repo_root import repo_root  # noqa: E402

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
CORPUS_DST = Path("/tmp/ledger_bisect_pin_corpus_outcomes.txt")
META_DST = Path("/tmp/ledger_bisect_witness_meta.json")


def expected_status(ledger_line: str) -> str:
    m = re.search(r"\bregressed-from:(\S+)", ledger_line)
    if m:
        return m.group(1)
    parts = ledger_line.split()
    return parts[1] if len(parts) > 1 else ""


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: pin_bisect_setup.py <witness> <pin_observed.txt>", file=sys.stderr)
        return 2
    witness, observed_path = sys.argv[1], Path(sys.argv[2])
    repo = repo_root()

    # Ledger at PIN (current checkout must be PIN).
    ledger = (repo / "tests/omega/corpus_outcomes.txt").read_text().splitlines()
    ledger_line = next((ln for ln in ledger if ln.startswith(witness + " ")), None)
    if not ledger_line:
        print(f"witness missing from PIN ledger: {witness}", file=sys.stderr)
        return 2
    shutil.copy2(repo / "tests/omega/corpus_outcomes.txt", CORPUS_DST)

    observed = parse_records(observed_path.read_text())
    if witness not in observed:
        print(f"witness missing from PIN observation: {witness}", file=sys.stderr)
        return 2
    obs = observed[witness]
    pin_status = obs["status"]
    pin_diag = (obs["diagnostics"] or [""])[0]
    meta = {
        "pin_commit": PIN,
        "witness": witness,
        "expected_status": expected_status(ledger_line),
        "pin_broken_status": pin_status,
        "pin_first_diagnostic": pin_diag,
        "pin_diag_norm": norm_diag(pin_diag),
        "tier": witness.split("/")[0],
    }
    META_DST.write_text(json.dumps(meta, indent=2))
    print(json.dumps(meta, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
