#!/usr/bin/env python3
"""Copy PIN ledger + witness broken/expected snapshot for fixed bisect."""
from __future__ import annotations

import json
import re
import shutil
import sys
from pathlib import Path

import os

REPO = Path(os.environ["OMEGA_REPO"]) if os.environ.get("OMEGA_REPO") else Path(__file__).resolve().parents[3]
PIN_LEDGER = Path(os.environ["PIN_LEDGER"]) if os.environ.get("PIN_LEDGER") else REPO / "tests/omega/corpus_outcomes.txt"
TOOLS = Path(__file__).resolve().parent


def parse_ledger_line(line: str) -> dict:
    parts = line.split()
    out = {"status": parts[1], "regressed_from": None, "extra": parts[2:] if len(parts) > 2 else []}
    for p in out["extra"]:
        if p.startswith("regressed-from:"):
            out["regressed_from"] = p.split(":", 1)[1].split("+")[0]
    if out["regressed_from"]:
        out["expected_status"] = out["regressed_from"]
    else:
        out["expected_status"] = out["status"]
    return out


def ledger_line(fixture: str, ledger_path: Path) -> str | None:
    for line in ledger_path.read_text().splitlines():
        if line.startswith(fixture + " "):
            return line
    return None


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_record(path: Path, fixture: str) -> tuple[str, str] | None:
    text = path.read_text() if path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(fixture + " ")), None)
    if not line:
        return None
    parts = line.split()
    if len(parts) < 3:
        return None
    status = parts[1]
    diag = ""
    for l in text.splitlines():
        if l.startswith("\t") and diag == "":
            # first diagnostic after fixture line — scan properly
            pass
    lines = text.splitlines()
    for i, l in enumerate(lines):
        if l.startswith(fixture + " "):
            if i + 1 < len(lines) and lines[i + 1].startswith("\t"):
                diag = lines[i + 1].lstrip("\t").strip()
            break
    return status, diag


def main() -> int:
    witness = sys.argv[1]
    pin_record = Path(sys.argv[2])
    out_dir = Path(sys.argv[3])
    out_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(PIN_LEDGER, out_dir / "corpus_outcomes.txt")
    led = ledger_line(witness, PIN_LEDGER)
    if not led:
        print(f"missing ledger row for {witness}", file=sys.stderr)
        return 1
    meta = parse_ledger_line(led)
    obs = parse_record(pin_record, witness)
    if not obs:
        print(f"missing PIN observation for {witness}", file=sys.stderr)
        return 1
    broken_status, broken_diag = obs
    snap = {
        "witness": witness,
        "pin_commit": subprocess_rev_parse(),
        "expected_status": meta["expected_status"],
        "ledger_status": meta["status"],
        "broken_status": broken_status,
        "broken_diag": broken_diag,
        "broken_diag_norm": norm_diag(broken_diag),
    }
    (out_dir / "pin_observation.json").write_text(json.dumps(snap, indent=2) + "\n")
    print(json.dumps(snap, indent=2))
    return 0


def subprocess_rev_parse() -> str:
    import subprocess

    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip()


if __name__ == "__main__":
    raise SystemExit(main())
