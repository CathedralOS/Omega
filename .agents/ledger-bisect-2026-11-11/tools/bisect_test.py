#!/usr/bin/env python3
"""Fixed-PIN bisect oracle.

0  witness meets the PIN ledger expectation
1  witness reproduces the PIN status and the PIN diagnostic class
125 any other outcome, missing record, or unreadable input

Diagnostic class is the first diagnostic with quoted spans and numbers
normalized. A different failure is not exit 1.
"""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

RANK = {"incomplete": 0, "rejected": 0, "checked": 1, "built": 2, "timeout": -1}


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_records(text: str) -> dict[str, dict]:
    records: dict[str, dict] = {}
    current: str | None = None
    for line in text.splitlines():
        if line.startswith("\t") and current:
            records[current]["diagnostics"].append(line[1:].strip())
            continue
        if not line.strip() or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) < 3 or not parts[2].endswith("ms"):
            continue
        fixture, status = parts[0], parts[1]
        flags = [p for p in parts[3:] if not p.startswith("@") and not p.endswith("ms")]
        expected = None
        if "expected" in flags:
            expected = True
        elif "unexpected" in flags:
            expected = False
        records[fixture] = {
            "status": status,
            "expected_satisfied": expected,
            "diagnostics": [],
        }
        current = fixture
    return records


def meets_expected(tier: str, expected: str, status: str, expected_satisfied: bool | None) -> bool:
    if tier == "fail":
        return status in ("rejected", "incomplete")
    if RANK.get(status, -1) >= RANK.get(expected, -1):
        if expected_satisfied is False:
            return False
        return True
    return False


def main() -> int:
    witness = os.environ.get("BISECT_WITNESS", "")
    meta_path = os.environ.get("BISECT_WITNESS_PIN", "")
    rec_path = Path(sys.argv[1] if len(sys.argv) > 1 else "/tmp/bisect_one_record.txt")
    sha = os.environ.get("BISECT_SHA", "")
    log_path = os.environ.get("BISECT_STEP_LOG", "")
    status, diag, code = "", "", 125
    if witness and meta_path and rec_path.exists():
        meta = json.loads(Path(meta_path).read_text())
        records = parse_records(rec_path.read_text())
        obs = records.get(witness)
        if obs:
            status = obs["status"]
            diag = (obs.get("diagnostics") or [""])[0]
            if meets_expected(
                meta["tier"], meta["expected_status"], status, obs.get("expected_satisfied")
            ):
                code = 0
            elif status == meta["pin_status"] and norm_diag(diag) == meta["pin_diag_norm"]:
                code = 1
            else:
                code = 125
    payload = {"sha": sha, "code": code, "status": status, "diag": diag}
    Path("/tmp/bisect-last.json").write_text(json.dumps(payload))
    if log_path and sha:
        safe = diag.replace("\t", " ").replace("\n", " ")
        with open(log_path, "a", encoding="utf-8") as fh:
            fh.write(f"{sha}\t{code}\t{status}\t{safe}\n")
    return code


if __name__ == "__main__":
    raise SystemExit(main())
