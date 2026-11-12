#!/usr/bin/env python3
"""Bisect test against PIN-fixed ledger and witness observation (not checkout ledger)."""
from __future__ import annotations

import json
import sys
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
from classify_corpus import norm_diag  # noqa: E402


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1]


def first_diag(record_path: Path, witness: str) -> str:
    text = record_path.read_text() if record_path.exists() else ""
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if line.startswith(witness + " "):
            if i + 1 < len(lines) and lines[i + 1].startswith("\t"):
                return lines[i + 1][1:].strip()
            return ""
    return ""


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    meta_path = Path(f"/tmp/bisect_pin_meta/{witness.replace('/', '_')}.json")
    if not meta_path.exists():
        return 125
    meta = json.loads(meta_path.read_text())
    line = next(
        (l for l in rec_path.read_text().splitlines() if l.startswith(witness + " ")), None
    ) if rec_path.exists() else None
    if not line:
        return 125
    _fixture, status = parse_header(line) or (witness, "")
    if not status:
        return 125
    diag = first_diag(rec_path, witness)
    exp = meta["expected_status"]
    pin_status = meta["observed_status"]
    pin_norm = meta["observed_diag_norm"]
    if status == pin_status and (
        diag == meta["observed_diagnostic"]
        or (diag and pin_norm and norm_diag(diag) == pin_norm)
    ):
        return 1
    if exp and status == exp:
        return 0
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
