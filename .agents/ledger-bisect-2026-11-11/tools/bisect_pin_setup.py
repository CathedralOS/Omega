#!/usr/bin/env python3
"""Materialize /tmp/bisect-pin fixtures for a witness from PIN measurement."""
from __future__ import annotations

import json
import re
import shutil
import sys
from pathlib import Path

PIN = Path("/agent/repos/omega")
PIN_OUTCOMES = PIN / "tests/omega/corpus_outcomes.txt"
OBS = Path("/tmp/part-12-pin.txt")
OUT_DIR = Path("/tmp/bisect-pin")


def parse_records(text: str) -> dict[str, dict]:
    records: dict[str, dict] = {}
    current: str | None = None
    for line in text.splitlines():
        if line.startswith("\t") and current:
            records[current]["diagnostics"].append(line[1:].strip())
            continue
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) < 3 or not parts[2].endswith("ms"):
            continue
        fixture, status = parts[0], parts[1]
        records[fixture] = {
            "fixture": fixture,
            "status": status,
            "diagnostics": [],
            "flags": [p for p in parts[3:] if not p.startswith("@") and not p.endswith("ms")],
        }
        current = fixture
    return records


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def ledger_expected_status(line: str) -> str:
    parts = line.split()
    if len(parts) < 2:
        raise ValueError(line)
    status = parts[1]
    for p in parts[3:]:
        if p.startswith("regressed-from:"):
            return p.split(":", 1)[1]
    return status


def main() -> int:
    witness = sys.argv[1]
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    shutil.copy2(PIN_OUTCOMES, OUT_DIR / "corpus_outcomes.txt")
    obs = parse_records(OBS.read_text())
    if witness not in obs:
        print(f"missing witness in PIN observation: {witness}", file=sys.stderr)
        return 1
    ledger_line = next(
        (l for l in PIN_OUTCOMES.read_text().splitlines() if l.startswith(witness + " ")), None
    )
    if not ledger_line:
        print(f"missing witness in PIN ledger: {witness}", file=sys.stderr)
        return 1
    o = obs[witness]
    bad_diag = (o["diagnostics"] or [""])[0]
    spec = {
        "witness": witness,
        "bad_status": o["status"],
        "bad_diag": bad_diag,
        "bad_diag_norm": norm_diag(bad_diag),
        "good_status": ledger_expected_status(ledger_line),
        "ledger_line": ledger_line.strip(),
    }
    specs_path = OUT_DIR / "witnesses.json"
    specs: dict = {}
    if specs_path.exists():
        specs = json.loads(specs_path.read_text())
    specs[witness] = spec
    specs_path.write_text(json.dumps(specs, indent=2) + "\n")
    print(json.dumps(spec, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
