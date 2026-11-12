#!/usr/bin/env python3
"""Freeze PIN ledger + witness broken observation for bisect (run on PIN only)."""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
LEDGER_SRC = REPO / "tests/omega/corpus_outcomes.txt"
LEDGER_PIN = Path("/tmp/pin_corpus_outcomes.txt")
OBS_PATH = Path("/tmp/pin_witness_observation.json")


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_record(text: str, witness: str) -> tuple[str, str, list[str]] | None:
    cur = None
    status = ""
    diags: list[str] = []
    for line in text.splitlines():
        if line.startswith("\t"):
            if cur == witness:
                diags.append(line[1:].strip())
            continue
        if cur == witness:
            return cur, status, diags
        parts = line.split()
        if len(parts) >= 3 and parts[2].endswith("ms"):
            cur = parts[0]
            status = parts[1]
            diags = []
    if cur == witness:
        return cur, status, diags
    return None


def expected_good_from_ledger_line(line: str) -> str | None:
    parts = line.split()
    if len(parts) < 2:
        return None
    for tok in parts[3:]:
        if tok.startswith("regressed-from:"):
            return tok.split(":", 1)[1]
    return parts[1]


def ledger_line(witness: str, ledger_text: str) -> str | None:
    for line in ledger_text.splitlines():
        if line.startswith(witness + " "):
            return line
    return None


def run_witness(witness: str) -> str:
    tools = Path(__file__).resolve().parent
    import os
    rec = Path("/tmp/pin_witness_record.txt")
    root = str(REPO)
    runner = subprocess.run(
        f'ls -t "{root}/target/debug/build/compiler/"*/out/corpus_runner-* 2>/dev/null | head -1',
        shell=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if not runner:
        raise SystemExit("corpus_runner missing; build compiler test corpus_runner")
    subprocess.run(
        [
            "nice",
            "-n",
            "19",
            runner,
        ],
        env={
            **os.environ,
            "OMEGA_CORPUS_ROOT": root,
            "OMEGA_LIBRARY_ROOT": str(REPO / "source/library"),
            "OMEGA_CORPUS_JOBS": "1",
            "OMEGA_CORPUS_FIXTURE_FILTER": witness,
            "OMEGA_CORPUS_RECORDS": str(rec),
        },
        stdin=subprocess.DEVNULL,
        check=False,
    )
    return rec.read_text() if rec.exists() else ""


def main() -> int:
    witness = sys.argv[1]
    measure_path = Path(sys.argv[2]) if len(sys.argv) > 2 else None
    head = subprocess.run(
        ["git", "-C", str(REPO), "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    if head != PIN:
        print(f"expected PIN {PIN[:12]}, got {head[:12]}", file=sys.stderr)

    LEDGER_PIN.write_text(LEDGER_SRC.read_text())
    ledger_text = LEDGER_PIN.read_text()
    line = ledger_line(witness, ledger_text)
    expected = expected_good_from_ledger_line(line) if line else None
    tier = witness.split("/")[0]
    if expected is None and tier == "pass":
        expected = "checked"
    elif expected is None and tier == "fail":
        expected = "rejected"

    if measure_path and measure_path.exists():
        obs_text = measure_path.read_text()
    else:
        obs_text = run_witness(witness)

    parsed = parse_record(obs_text, witness)
    if not parsed:
        print("no PIN observation for witness", file=sys.stderr)
        return 1
    _, bad_status, diags = parsed
    first_diag = diags[0] if diags else ""
    payload = {
        "witness": witness,
        "pin_commit": PIN,
        "expected_good_status": expected,
        "pin_bad_status": bad_status,
        "pin_bad_diag": first_diag,
        "pin_bad_diag_norm": norm_diag(first_diag),
        "tier": tier,
    }
    OBS_PATH.write_text(json.dumps(payload, indent=2) + "\n")
    print(json.dumps(payload, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
