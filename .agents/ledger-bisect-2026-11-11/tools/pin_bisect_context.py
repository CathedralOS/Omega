#!/usr/bin/env python3
"""Install fixed PIN ledger + witness observation under /tmp for bisect."""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN_SHA = os.environ.get("BISECT_PIN", "51e8c45c793007f211d0d9baee7de16767172444")
PIN_LEDGER = Path("/tmp/ledger-bisect-corpus_outcomes.txt")
CTX = Path("/tmp/ledger-bisect-context.json")
RECORD = Path("/tmp/ledger-bisect-pin-record.txt")


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


def ledger_expected(witness: str, ledger_text: str) -> str | None:
    for line in ledger_text.splitlines():
        if line.startswith(witness + " "):
            return line.split()[1]
    return None


def regressed_from_status(witness: str) -> str | None:
    """Optional sweep annotation: regressed-from:<status> on the ledger line."""
    for line in PIN_LEDGER.read_text().splitlines():
        if not line.startswith(witness + " "):
            continue
        for tok in line.split():
            if tok.startswith("regressed-from:"):
                return tok.split(":", 1)[1]
    return None


def pin_ledger_text() -> str:
    shown = subprocess.run(
        ["git", "show", f"{PIN_SHA}:tests/omega/corpus_outcomes.txt"],
        capture_output=True,
        text=True,
        cwd=REPO,
    )
    if shown.returncode == 0 and shown.stdout:
        return shown.stdout
    return (REPO / "tests/omega/corpus_outcomes.txt").read_text()


def capture_pin_observation(witness: str, measure_file: Path | None = None) -> None:
    PIN_LEDGER.write_text(pin_ledger_text())
    if measure_file and measure_file.is_file():
        parsed = parse_record(measure_file.read_text(), witness)
        if not parsed:
            raise SystemExit(f"no measure row for {witness} in {measure_file}")
        pin_status, pin_diags = parsed
    else:
        env = {
            **dict(os.environ),
            "OMEGA_CORPUS_ROOT": str(REPO),
            "OMEGA_LIBRARY_ROOT": str(REPO / "source/library"),
            "OMEGA_CORPUS_JOBS": "1",
            "OMEGA_CORPUS_FIXTURE_FILTER": witness,
            "OMEGA_CORPUS_RECORDS": str(RECORD),
        }
        subprocess.run(["rm", "-f", str(RECORD)], check=False)
        runner = sorted(
            (REPO / "target/debug/build/compiler").glob("*/out/corpus_runner-*"),
            key=lambda p: p.stat().st_mtime,
            reverse=True,
        )
        if not runner:
            raise SystemExit("corpus_runner missing; build compiler test target first")
        subprocess.run([str(runner[0])], env=env, stdin=subprocess.DEVNULL, check=False)
        parsed = parse_record(RECORD.read_text() if RECORD.exists() else "", witness)
        if not parsed:
            raise SystemExit(f"no PIN record for {witness}")
        pin_status, pin_diags = parsed
    ledger_text = PIN_LEDGER.read_text()
    expected = regressed_from_status(witness) or ledger_expected(witness, ledger_text)
    if expected is None:
        raise SystemExit(f"no ledger row for {witness}")
    tier = witness.split("/")[0]
    payload = {
        "witness": witness,
        "pin_commit": PIN_SHA,
        "pin_status": pin_status,
        "pin_first_diag": (pin_diags or [""])[0],
        "pin_diag_norm": norm_diag((pin_diags or [""])[0]),
        "ledger_expected_status": expected,
        "tier": tier,
    }
    CTX.write_text(json.dumps(payload, indent=2) + "\n")
    print(json.dumps(payload, indent=2))


def main() -> int:
    if len(sys.argv) not in (2, 3):
        print("usage: pin_bisect_context.py <witness> [pin_measure.txt]", file=sys.stderr)
        return 2
    measure = Path(sys.argv[2]) if len(sys.argv) == 3 else None
    capture_pin_observation(sys.argv[1], measure)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
