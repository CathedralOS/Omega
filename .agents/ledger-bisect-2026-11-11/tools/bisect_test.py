#!/usr/bin/env python3
"""Classify one witness run vs frozen PIN reference (not checkout ledger)."""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

REF_ROOT = Path(os.environ["BISECT_PIN_REF"])


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_header(line: str) -> tuple[str, str] | None:
    parts = line.split()
    if len(parts) < 3:
        return None
    return parts[0], parts[1]


def first_diagnostic(text: str, fixture: str) -> str:
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if line.startswith(fixture + " "):
            for follow in lines[i + 1 :]:
                if follow.startswith("\t"):
                    return follow[1:].strip()
                if follow.strip() and not follow.startswith("#"):
                    break
            return ""
    return ""


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    safe = witness.replace("/", "__")
    ref = json.loads((REF_ROOT / f"{safe}.json").read_text(encoding="utf-8"))
    text = rec_path.read_text(encoding="utf-8") if rec_path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return 125
    fixture, status = parse_header(line)
    if fixture != witness:
        return 125
    diag = first_diagnostic(text, witness)
    diag_norm = norm_diag(diag) if diag else ref["pin_diag_norm"]

    expected = ref["expected_status"]
    pin_status = ref["pin_broken_status"]
    pin_norm = ref["pin_diag_norm"]

    if status == expected:
        return 0

    same_class = diag_norm == pin_norm or (pin_norm[:48] == diag_norm[:48])
    if status == pin_status and same_class:
        return 1

    return 125


if __name__ == "__main__":
    raise SystemExit(main())
