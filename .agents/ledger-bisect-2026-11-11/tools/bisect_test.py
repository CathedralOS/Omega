#!/usr/bin/env python3
"""Classify one corpus_runner record vs fixed PIN ledger + PIN broken observation."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

CTX = Path("/tmp/ledger-bisect-context.json")


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_record(text: str, witness: str) -> tuple[str, list[str]] | None:
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if line.startswith(witness + " "):
            parts = line.split()
            if len(parts) < 3:
                return None
            diags = []
            j = i + 1
            while j < len(lines) and lines[j].startswith("\t"):
                diags.append(lines[j][1:].strip())
                j += 1
            return parts[1], diags
    return None


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    if not CTX.is_file():
        return 125
    ctx = json.loads(CTX.read_text())
    if ctx["witness"] != witness:
        return 125
    text = rec_path.read_text() if rec_path.exists() else ""
    parsed = parse_record(text, witness)
    if not parsed:
        return 125
    status, diags = parsed
    first = (diags or [""])[0]
    expected = ctx["ledger_expected_status"]
    pin_status = ctx["pin_status"]
    pin_norm = ctx["pin_diag_norm"]

    tier = ctx["tier"]
    tier_good = {
        "pass": frozenset({"pass", "checked", "run"}),
        "run": frozenset({"run", "checked", "pass"}),
        "fail": frozenset({"fail", "rejected"}),
    }

    if status == pin_status and norm_diag(first) == pin_norm:
        return 1

    if status in tier_good.get(tier, frozenset({expected})) and norm_diag(first) != pin_norm:
        return 0

    if status == expected:
        return 0

    return 125


if __name__ == "__main__":
    raise SystemExit(main())
