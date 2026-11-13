#!/usr/bin/env python3
"""Materialize /tmp bisect expectation for one witness (PIN ledger + PIN observation)."""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402

REPO = Path(__file__).resolve().parents[3]
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
PIN_LEDGER = Path(os.environ.get("BISECT_PIN_LEDGER", "/tmp/bisect_pin_corpus_outcomes.txt"))
PIN_OBS = Path(
    os.environ.get(
        "BISECT_PIN_OBSERVED",
        Path(__file__).resolve().parents[1] / "pin_observed.txt",
    )
)
OUT_DIR = Path("/tmp")


def witness_slug(witness: str) -> str:
    return re.sub(r"[^a-zA-Z0-9_.-]+", "_", witness)


def ledger_expectation(fixture: str, ledger_text: str) -> tuple[str, str, str, str]:
    """Return (good_status, tier, ledger_status, ledger_diag_norm) from the PIN ledger."""
    recs = parse_records(ledger_text)
    rec = recs[fixture]
    tier = rec["tier"]
    raw = next((l for l in ledger_text.splitlines() if l.startswith(fixture + " ")), "")
    parts = raw.split()
    flags = [p for p in parts[3:] if not p.startswith("@") and not p.endswith("ms")]
    good = rec["status"]
    for flag in flags:
        if flag.startswith("regressed-from:"):
            good = flag.split(":", 1)[1].split("+", 1)[0]
            break
    diag = norm_diag((rec.get("diagnostics") or [""])[0])
    return good, tier, rec["status"], diag


def expected_fragments(fixture: str) -> list[str]:
    """PIN fixture oracle. Read from the commit, not the moving worktree."""
    rel = f"tests/omega/{fixture}/expected.txt"
    proc = subprocess.run(
        ["git", "-C", str(REPO), "show", f"{PIN}:{rel}"],
        text=True,
        capture_output=True,
    )
    if proc.returncode != 0:
        return []
    return [line.strip() for line in proc.stdout.splitlines() if line.strip()]


def pin_bad(fixture: str, obs_text: str) -> dict:
    recs = parse_records(obs_text)
    rec = recs[fixture]
    raw = next((l for l in obs_text.splitlines() if l.startswith(fixture + " ")), "")
    parts = raw.split()
    flags = [p for p in parts[3:] if not p.startswith("@") and not p.endswith("ms")]
    diag = (rec["diagnostics"] or [""])[0]
    return {
        "pin_bad_status": rec["status"],
        "pin_bad_diag": diag,
        "pin_bad_diag_norm": norm_diag(diag),
        "pin_bad_unexpected": "unexpected" in flags,
    }


def main() -> int:
    witness = sys.argv[1]
    ledger_text = PIN_LEDGER.read_text()
    obs_text = PIN_OBS.read_text()
    good_status, tier, ledger_status, ledger_diag_norm = ledger_expectation(witness, ledger_text)
    bad = pin_bad(witness, obs_text)
    payload = {
        "witness": witness,
        "tier": tier,
        "good_status": good_status,
        "ledger_status": ledger_status,
        "ledger_diag_norm": ledger_diag_norm,
        "expected_fragments": expected_fragments(witness),
        **bad,
    }
    out = OUT_DIR / f"bisect_expect_{witness_slug(witness)}.json"
    out.write_text(json.dumps(payload, indent=2))
    print(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
