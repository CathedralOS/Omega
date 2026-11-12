#!/usr/bin/env python3
"""Classify fixture rows vs fixed PIN ledger (good=0, bad=1, skip=125)."""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from bisect_test import meets_ledger_expected, matches_pin_regression  # noqa: E402
from classify_corpus import parse_records  # noqa: E402

REPO = Path(__file__).resolve().parents[3]
PIN_LEDGER = Path("/tmp/ledger-bisect-corpus_outcomes.txt")
REC = Path("/tmp/verify_corpus_record.txt")


def ledger_meta(fixture: str) -> dict:
    line = next(
        (ln for ln in PIN_LEDGER.read_text().splitlines() if ln.startswith(fixture + " ")),
        None,
    )
    if not line:
        return {}
    parts = line.split()
    tier = fixture.split("/")[0]
    want = parts[1]
    for tok in parts:
        if tok.startswith("regressed-from:"):
            want = tok.split(":", 1)[1]
    return {"fixture": fixture, "tier": tier, "ledger_expected_status": want}


def classify_fixture(fixture: str, records: dict) -> int:
    meta = ledger_meta(fixture)
    if fixture not in records or not meta:
        return 125
    obs = records[fixture]
    if meets_ledger_expected(obs["status"], meta):
        return 0
    if matches_pin_regression(obs, {**meta, "pin_status": "rejected", "pin_diag_norm": ""}):
        return 1
    meta2 = meta.copy()
    pin_obs = None
    pin_path = REPO / ".agents/ledger-bisect-2026-11-11/pin_full.txt"
    if pin_path.exists():
        pin_records = parse_records(pin_path.read_text())
        if fixture in pin_records:
            pin_obs = pin_records[fixture]
    if pin_obs:
        from classify_corpus import norm_diag

        meta2 |= {
            "pin_status": pin_obs["status"],
            "pin_diag_norm": norm_diag((pin_obs["diagnostics"] or [""])[0]),
        }
        if matches_pin_regression(obs, meta2):
            return 1
    # still worse than ledger for pass tier
    tier = meta["tier"]
    if tier != "fail" and obs["status"] not in ("built", "checked"):
        return 1
    return 125


def run_witness(witness: str) -> int:
    subprocess.run(
        [str(REPO / ".agents/ledger-bisect-2026-11-11/tools/prepare_bisect_pin.py"), witness],
        check=True,
        cwd=REPO,
        capture_output=True,
    )
    r = subprocess.run(
        [str(REPO / ".agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh")],
        cwd=REPO,
        env={**subprocess.os.environ, "BISECT_WITNESS": witness},
    )
    return r.returncode


def main() -> int:
    commit = sys.argv[1]
    witness = sys.argv[2]
    members = sys.argv[3:]
    subprocess.run(["git", "checkout", "-q", commit], cwd=REPO, check=True)
    ledger_src = REPO / "tests/omega/corpus_outcomes.txt"
    PIN_LEDGER.write_text(ledger_src.read_text())
    # witness triplet
    witness_rcs = [run_witness(witness) for _ in range(3)]
    # members batch
    subprocess.run(
        [
            str(REPO / ".agents/ledger-bisect-2026-11-11/run_pin_corpus.sh"),
            "/tmp/verify_members.txt",
        ],
        cwd=REPO,
        check=False,
        env={
            **subprocess.os.environ,
            "OMEGA_CORPUS_FIXTURE_FILTER": ",".join(members),
            "OMEGA_CORPUS_RECORDS": str(REC),
        },
    )
    records = parse_records(REC.read_text()) if REC.exists() else {}
    member_rc = {m: classify_fixture(m, records) for m in members}
    out = {
        "commit": commit,
        "witness_rcs": witness_rcs,
        "member_rc": member_rc,
    }
    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
