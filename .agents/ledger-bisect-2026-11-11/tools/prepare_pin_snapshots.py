#!/usr/bin/env python3
"""Copy PIN ledger and write per-witness observation snapshots for bisect."""
from __future__ import annotations

import json
import shutil
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402

SNAP = Path("/tmp/ledger-bisect-pin")
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
OBS = REPO / ".agents/ledger-bisect-2026-11-11/pin_observed.txt"


def main() -> int:
    SNAP.mkdir(parents=True, exist_ok=True)
    shutil.copy2(REPO / "tests/omega/corpus_outcomes.txt", SNAP / "corpus_outcomes.txt")
    (SNAP / "pin_commit.txt").write_text(PIN + "\n")
    observed = parse_records(OBS.read_text())
    witnesses = sys.argv[1:]
    for w in witnesses:
        if w not in observed:
            raise SystemExit(f"missing PIN observation for {w}")
        o = observed[w]
        diag = (o["diagnostics"] or [""])[0]
        payload = {
            "witness": w,
            "pin_status": o["status"],
            "pin_first_diagnostic": diag,
            "pin_diag_norm": norm_diag(diag),
        }
        out = SNAP / f"witness_{w.replace('/', '__')}.json"
        out.write_text(json.dumps(payload, indent=2))
        print(f"wrote {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
