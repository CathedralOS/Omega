#!/usr/bin/env python3
"""Build fixed PIN bisect context for one witness (ledger + broken observation)."""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

_REPO = os.environ.get("LEDGER_BISECT_ROOT")
REPO = Path(_REPO) if _REPO else Path(__file__).resolve().parents[3]
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
CTX = Path("/tmp/ledger-bisect-pin-context.json")
LEDGER_COPY = Path("/tmp/ledger-bisect-corpus_outcomes.txt")


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_header_line(line: str) -> dict | None:
    parts = line.split()
    if len(parts) < 3 or not parts[2].endswith("ms"):
        return None
    fixture, status = parts[0], parts[1]
    tier = fixture.split("/")[0]
    flags = [p for p in parts[3:] if not p.startswith("@") and not p.endswith("ms")]
    regressed_from = None
    for f in flags:
        if f.startswith("regressed-from:"):
            regressed_from = f.split(":", 1)[1]
    expect_fail = "expected" in flags
    expect_pass = "unexpected" in flags
    return {
        "fixture": fixture,
        "tier": tier,
        "ledger_status": status,
        "flags": flags,
        "regressed_from": regressed_from,
        "expect_fail": expect_fail,
        "expect_pass": expect_pass,
    }


def parse_record_block(text: str, fixture: str) -> dict:
    header = None
    diags: list[str] = []
    for line in text.splitlines():
        if line.startswith(fixture + " "):
            header = line
            continue
        if header and line.startswith("\t"):
            diags.append(line[1:].strip())
        elif header and line.strip() and not line.startswith("\t"):
            break
    if not header:
        raise SystemExit(f"missing record for {fixture}")
    parts = parse_header_line(header)
    if not parts:
        raise SystemExit(f"unparsed header for {fixture}")
    first = diags[0] if diags else ""
    return {
        **parts,
        "observed_status": parts["ledger_status"],
        "first_diagnostic": first,
        "diag_norm": norm_diag(first) if first else "",
        "diagnostics": diags,
    }


def expected_good_status(ledger_hdr: dict) -> str:
    if ledger_hdr.get("regressed_from"):
        return ledger_hdr["regressed_from"]
    tier = ledger_hdr["tier"]
    if tier == "fail":
        return ledger_hdr["ledger_status"]
    return ledger_hdr["ledger_status"]


def main() -> int:
    witness = sys.argv[1]
    record_path = Path(sys.argv[2]) if len(sys.argv) > 2 else Path("/tmp/bisect_one_record.txt")
    ledger_src = REPO / "tests/omega/corpus_outcomes.txt"
    LEDGER_COPY.write_text(ledger_src.read_text())
    obs = parse_record_block(record_path.read_text(), witness)
    led = parse_record_block(LEDGER_COPY.read_text(), witness)
    good_status = expected_good_status(led)
    ctx = {
        "pin": PIN,
        "witness": witness,
        "ledger_copy": str(LEDGER_COPY),
        "pin_broken": {
            "status": obs["observed_status"],
            "first_diagnostic": obs["first_diagnostic"],
            "diag_norm": obs["diag_norm"],
        },
        "pin_expected": {
            "status": good_status,
            "tier": led["tier"],
            "expect_fail": led["expect_fail"],
            "expect_pass": led["expect_pass"],
        },
    }
    CTX.write_text(json.dumps(ctx, indent=2))
    print(CTX)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
