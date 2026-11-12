#!/usr/bin/env python3
"""Compare one corpus record against fixed PIN ledger + PIN witness observation."""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

LEDGER_PIN = Path("/tmp/ledger_bisect_pin_corpus_outcomes.txt")
PIN_OBS = Path("/tmp/ledger_bisect_witness_pin.json")


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


def ledger_line(fixture: str) -> str | None:
    if not LEDGER_PIN.exists():
        return None
    for line in LEDGER_PIN.read_text().splitlines():
        if line.startswith(fixture + " "):
            return line
    return None


def expected_outcome(ledger: str) -> str:
    parts = ledger.split()
    for tok in parts:
        if tok.startswith("regressed-from:"):
            return tok.split(":", 1)[1]
    return parts[1]


def first_diagnostic(text: str, fixture: str) -> str:
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if line.startswith(fixture + " "):
            if i + 1 < len(lines) and lines[i + 1].startswith("\t"):
                return lines[i + 1][1:].strip()
    return ""


def parse_record(rec_path: Path, witness: str) -> tuple[str, str] | None:
    if not rec_path.exists():
        return None
    text = rec_path.read_text()
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return None
    parsed = parse_header(line)
    if not parsed:
        return None
    return parsed[1], first_diagnostic(text, witness)


def status_matches(expected: str, observed: str, tier: str) -> bool:
    if observed == expected:
        return True
    if tier == "pass" and expected in ("checked", "built") and observed in ("checked", "built"):
        return True
    if tier == "fail" and expected in ("rejected", "incomplete"):
        return observed in ("rejected", "incomplete")
    return False


def main() -> int:
    witness = os.environ.get("BISECT_WITNESS", sys.argv[1] if len(sys.argv) > 1 else "")
    rec_path = Path(sys.argv[-1])
    if not witness or not PIN_OBS.exists():
        return 125
    pin = json.loads(PIN_OBS.read_text())
    if pin.get("witness") != witness:
        return 125
    led = ledger_line(witness)
    if not led:
        return 125
    expected = expected_outcome(led)
    tier = witness.split("/")[0]
    parsed = parse_record(rec_path, witness)
    if not parsed:
        return 125
    observed_status, observed_diag = parsed
    pin_status = pin["status"]
    pin_diag_norm = pin["diag_norm"]

    if status_matches(expected, observed_status, tier):
        return 0

    pin_broken = not status_matches(expected, pin_status, tier)
    same_class = norm_diag(observed_diag) == pin_diag_norm
    same_exact = (
        observed_status == pin_status
        and (not pin.get("diag") or observed_diag == pin["diag"])
    )
    if pin_broken and (same_exact or same_class):
        return 1

    return 125


if __name__ == "__main__":
    raise SystemExit(main())
