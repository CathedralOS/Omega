#!/usr/bin/env python3
"""Write /tmp/ledger-bisect-pin/witnesses/<slug>.json from PIN corpus records."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402

BASE = Path("/tmp/ledger-bisect-pin")
LEDGER = BASE / "corpus_outcomes.txt"
WDIR = BASE / "witnesses"


def expected_status(line: str) -> str:
    for tok in line.split():
        if tok.startswith("regressed-from:"):
            return tok.split(":", 1)[1]
    return line.split()[1]


def ledger_line(fixture: str) -> str | None:
    for line in LEDGER.read_text().splitlines():
        if line.startswith(fixture + " "):
            return line
    return None


def slug(fixture: str) -> str:
    return fixture.replace("/", "__")


def write_one(witness: str, observed_path: Path) -> None:
    obs = parse_records(observed_path.read_text())
    if witness not in obs:
        raise SystemExit(f"missing PIN observation for {witness}")
    ll = ledger_line(witness)
    if not ll:
        raise SystemExit(f"missing ledger row for {witness}")
    diag = (obs[witness]["diagnostics"] or [""])[0]
    payload = {
        "witness": witness,
        "tier": witness.split("/")[0],
        "expected_status": expected_status(ll),
        "pin_status": obs[witness]["status"],
        "pin_diag": diag,
        "pin_diag_norm": norm_diag(diag),
    }
    WDIR.mkdir(parents=True, exist_ok=True)
    (WDIR / f"{slug(witness)}.json").write_text(json.dumps(payload, indent=2))


def main() -> int:
    observed = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/tmp/ledger-audit/part0_pin_observed.txt")
    witnesses = sys.argv[2:]
    if not witnesses:
        state = Path(__file__).resolve().parents[1] / "state.json"
        if state.exists():
            data = json.loads(state.read_text())
            witnesses = [c["witness"] for c in data.get("pin_clusters", [])]
    for w in witnesses:
        write_one(w, observed)
        print(f"wrote {WDIR/slug(w)}.json expected={json.loads((WDIR/f'{slug(w)}.json').read_text())['expected_status']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
