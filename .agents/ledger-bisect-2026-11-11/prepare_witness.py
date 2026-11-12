#!/usr/bin/env python3
"""Snapshot PIN ledger row + PIN corpus observation for one witness."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path


def parse_records(text: str) -> dict[str, dict]:
    records: dict[str, dict] = {}
    current_key: str | None = None
    for line in text.splitlines():
        if line.startswith("\t") and current_key:
            records[current_key]["diagnostics"].append(line[1:].strip())
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
        }
        current_key = fixture
    return records


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_regressed_from(flags: list[str]) -> str | None:
    for f in flags:
        if f.startswith("regressed-from:"):
            return f.split(":", 1)[1].split("+")[0]
    return None


def parse_pin_observation(observed_path: Path, witness: str) -> dict:
    text = observed_path.read_text()
    recs = parse_records(text)
    if witness not in recs:
        raise SystemExit(f"missing PIN observation for {witness}")
    obs = recs[witness]
    diag = (obs["diagnostics"] or [""])[0]
    return {
        "pin_status": obs["status"],
        "pin_first_diagnostic": diag,
        "pin_diag_norm": norm_diag(diag) if diag else "",
    }


def main() -> int:
    witness, obs_path, out_path = sys.argv[1:4]
    golden = parse_records(Path("/tmp/bisect_pin_corpus_outcomes.txt").read_text())
    if witness not in golden:
        raise SystemExit(f"witness not in PIN ledger: {witness}")
    line = next(
        l
        for l in Path("/tmp/bisect_pin_corpus_outcomes.txt").read_text().splitlines()
        if l.startswith(witness + " ")
    )
    parts = line.split()
    flags = [p for p in parts[3:] if not p.endswith("ms") and not p.startswith("@")]
    regressed = parse_regressed_from(flags)
    ledger_status = parts[1]
    expected = regressed if regressed else ledger_status
    pin_obs = parse_pin_observation(Path(obs_path), witness)
    payload = {
        "witness": witness,
        "ledger_status": ledger_status,
        "expected_good_status": expected,
        "regressed_from": regressed,
        **pin_obs,
    }
    Path(out_path).write_text(json.dumps(payload, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
