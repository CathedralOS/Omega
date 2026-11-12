#!/usr/bin/env python3
"""Compare one fixture run against frozen PIN witness observation (not checkout ledger)."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

OBS_PATH = Path("/tmp/pin_witness_observation.json")
LEDGER_PIN = Path("/tmp/pin_corpus_outcomes.txt")


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_record(text: str, witness: str) -> tuple[str, str, list[str]] | None:
    cur = None
    diags: list[str] = []
    status = ""
    for line in text.splitlines():
        if line.startswith("\t"):
            if cur == witness:
                diags.append(line[1:].strip())
            continue
        parts = line.split()
        if len(parts) >= 3 and parts[2].endswith("ms"):
            if cur == witness:
                return witness, status, diags
            cur = parts[0]
            status = parts[1]
            diags = []
    if cur == witness:
        return witness, status, diags
    return None


def outcome_matches_good(status: str, expected: str, tier: str) -> bool:
    if status == expected:
        return True
    if tier == "pass" and expected == "checked" and status == "built":
        return True
    if tier == "fail" and expected in ("rejected", "incomplete"):
        return status in ("rejected", "incomplete")
    return False


def outcome_matches_bad(status: str, diag_norm: str, obs: dict) -> bool:
    if status != obs["pin_bad_status"]:
        return False
    if not obs.get("pin_bad_diag_norm"):
        return True
    if diag_norm == obs["pin_bad_diag_norm"]:
        return True
    # same class: normalized prefix agreement
    a, b = diag_norm, obs["pin_bad_diag_norm"]
    n = min(len(a), len(b), 80)
    return a[:n] == b[:n]


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    if not OBS_PATH.exists():
        return 125
    obs = json.loads(OBS_PATH.read_text())
    if witness != obs["witness"]:
        return 125
    text = rec_path.read_text() if rec_path.exists() else ""
    parsed = parse_record(text, witness)
    if not parsed:
        return 125
    _, status, diags = parsed
    first = diags[0] if diags else ""
    dn = norm_diag(first)
    tier = obs.get("tier", witness.split("/")[0])
    expected = obs["expected_good_status"]

    if outcome_matches_good(status, expected, tier):
        return 0
    if outcome_matches_bad(status, dn, obs):
        return 1
    if tier == "pass" and status == obs["pin_bad_status"]:
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
