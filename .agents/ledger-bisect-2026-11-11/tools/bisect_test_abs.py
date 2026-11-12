#!/usr/bin/env python3
"""Bisect helper when ledger row is missing: pass pass/run expect checked or built."""
import sys
from pathlib import Path


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1]


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    text = rec_path.read_text() if rec_path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return 125
    fixture, status = parse_header(line)
    tier = fixture.split("/")[0]
    if tier == "fail":
        good = status in ("rejected", "incomplete")
    else:
        good = status in ("checked", "built")
    return 0 if good else 1


if __name__ == "__main__":
    raise SystemExit(main())
