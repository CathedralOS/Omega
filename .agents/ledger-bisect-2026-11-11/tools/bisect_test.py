#!/usr/bin/env python3
"""Bisect step: fixed PIN good_status vs observed (not per-rev ledger)."""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1]


def main() -> int:
    if len(sys.argv) != 3:
        return 125
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    spec_path = Path(
        os.environ.get(
            "BISECT_PIN_SPEC",
            f"/tmp/ledger-bisect-pin/{witness.replace('/', '_')}.json",
        )
    )
    if not spec_path.is_file():
        return 125
    spec = json.loads(spec_path.read_text())
    if not rec_path.is_file():
        return 125
    header = next((l for l in rec_path.read_text().splitlines() if l.startswith(witness + " ")), None)
    if not header:
        return 125
    parsed = parse_header(header)
    if not parsed:
        return 125
    _, status = parsed
    good_status = spec["good_status"]
    if status == good_status:
        return 0
    if re.match(r"^(rejected|incomplete|checked|built)$", status):
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
