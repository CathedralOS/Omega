#!/usr/bin/env python3
"""Copy PIN ledger and build witness profile from PIN observation."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_record_text(text: str, fixture: str) -> dict | None:
    lines = text.splitlines()
    header = next((l for l in lines if l.startswith(fixture + " ")), None)
    if not header:
        return None
    parts = header.split()
    status = parts[1]
    tier = fixture.split("/")[0]
    diags: list[str] = []
    past_header = False
    for line in lines:
        if line.startswith(fixture + " "):
            past_header = True
            continue
        if past_header and line.startswith("\t"):
            diags.append(line[1:].strip())
        elif past_header and diags and line.strip():
            break
    return {"fixture": fixture, "tier": tier, "status": status, "diagnostics": diags}


def main() -> int:
    repo = Path(sys.argv[1])
    pin = sys.argv[2]
    observed = Path(sys.argv[3])
    witness = sys.argv[4]
    out_dir = Path(sys.argv[5])
    out_dir.mkdir(parents=True, exist_ok=True)
    ledger_dst = out_dir / "pin_corpus_outcomes.txt"
    ledger_dst.write_text((repo / "tests/omega/corpus_outcomes.txt").read_text())
    obs = parse_record_text(observed.read_text(), witness)
    if obs is None:
        raise SystemExit(f"witness {witness} missing from {observed}")
    profile = {
        "witness": witness,
        "pin_status": obs["status"],
        "pin_diag": obs["diagnostics"][0] if obs["diagnostics"] else "",
        "pin_diag_norm": norm_diag(obs["diagnostics"][0]) if obs["diagnostics"] else "",
        "pin": pin,
    }
    safe = witness.replace("/", "_")
    (out_dir / f"profile_{safe}.json").write_text(json.dumps(profile, indent=2))
    print(json.dumps(profile))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
