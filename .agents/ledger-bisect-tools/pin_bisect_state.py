#!/usr/bin/env python3
"""Prepare fixed PIN ledger and per-witness observations for bisect."""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(
    subprocess.check_output(
        ["git", "-C", str(Path(__file__).resolve().parent), "rev-parse", "--show-toplevel"]
    )
    .decode()
    .strip()
)
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
PIN_LEDGER = Path("/tmp/pin_corpus_outcomes.txt")
PIN_OBS = Path("/tmp/bisect_pin_obs.json")


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_records(text: str) -> dict[str, dict]:
    records: dict[str, dict] = {}
    cur: str | None = None
    for line in text.splitlines():
        if line.startswith("\t") and cur:
            records[cur]["diagnostics"].append(line[1:].strip())
            continue
        parts = line.split()
        if len(parts) >= 3 and parts[2].endswith("ms"):
            cur = parts[0]
            records[cur] = {"status": parts[1], "diagnostics": []}
    return records


def expected_good_status(ledger_line: str) -> str:
    parts = ledger_line.split()
    for p in parts:
        if p.startswith("regressed-from:"):
            payload = p[len("regressed-from:") :]
            return payload.split("+")[0]
    return parts[1]


def ledger_line(fixture: str, ledger_text: str) -> str | None:
    for line in ledger_text.splitlines():
        if line.startswith(fixture + " "):
            return line
    return None


def measure(witness: str) -> dict:
    script = REPO / ".agents/ledger-bisect-tools/measure_one.sh"
    rec = Path(f"/tmp/measure_{witness.replace('/', '_')}.txt")
    subprocess.run(["bash", str(script), witness], check=False, cwd=REPO)
    text = rec.read_text() if rec.exists() else ""
    recs = parse_records(text)
    if witness not in recs:
        raise SystemExit(f"no corpus record for {witness}")
    first = recs[witness]["diagnostics"][0] if recs[witness]["diagnostics"] else ""
    return {
        "pin_bad_status": recs[witness]["status"],
        "pin_bad_diag": first,
        "pin_bad_diag_norm": norm_diag(first),
        "tier": witness.split("/")[0],
    }


def main() -> int:
    witnesses = sys.argv[1:]
    if not witnesses:
        print("usage: pin_bisect_state.py <witness> ...", file=sys.stderr)
        return 2
    subprocess.run(["git", "-C", str(REPO), "checkout", PIN, "-q"], check=True)
    ledger_src = REPO / "tests/omega/corpus_outcomes.txt"
    PIN_LEDGER.write_text(ledger_src.read_text())
    ledger_text = PIN_LEDGER.read_text()
    state: dict[str, dict] = {}
    for w in witnesses:
        obs = measure(w)
        line = ledger_line(w, ledger_text)
        if not line:
            raise SystemExit(f"{w} missing from PIN ledger")
        obs["pin_good_status"] = expected_good_status(line)
        state[w] = obs
    PIN_OBS.write_text(json.dumps(state, indent=2))
    subprocess.run(["git", "-C", str(REPO), "checkout", "-"], check=False)
    print(f"wrote {PIN_LEDGER} and {PIN_OBS}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
