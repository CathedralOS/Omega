#!/usr/bin/env python3
"""Bisect step test: fixed PIN broken vs fixed PIN ledger good (not per-revision ledger)."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import norm_diag, parse_records  # noqa: E402

RANK = {"built": 2, "checked": 1}


def load_expect() -> dict:
    path = os.environ.get("BISECT_EXPECT_FILE")
    if not path:
        raise RuntimeError("BISECT_EXPECT_FILE not set")
    return json.loads(Path(path).read_text())


def record_flags(line: str) -> list[str]:
    parts = line.split()
    return [p for p in parts[3:] if not p.startswith("@") and not p.endswith("ms")]


def main() -> int:
    witness, rec_path = sys.argv[1], Path(sys.argv[2])
    expect = load_expect()
    if witness != expect["witness"]:
        return 125
    text = rec_path.read_text() if rec_path.exists() else ""
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return 125
    obs = parse_records(text + ("\n" if text and not text.endswith("\n") else ""))[witness]
    flags = record_flags(line)
    tier = expect["tier"]

    def pin_bad() -> bool:
        if obs["status"] != expect["pin_bad_status"]:
            return False
        if expect["pin_bad_unexpected"] != ("unexpected" in flags):
            return False
        diag = norm_diag((obs["diagnostics"] or [""])[0])
        return diag == expect["pin_bad_diag_norm"]

    def good() -> bool:
        if tier == "fail":
            if obs["status"] not in ("rejected", "incomplete"):
                return False
            if "unexpected" in flags:
                return False
            return True
        need = RANK.get(expect["good_status"], 0)
        got = RANK.get(obs["status"], 0)
        if got < need:
            return False
        if "unexpected" in flags:
            return False
        return True

    if pin_bad():
        return 1
    if good():
        return 0
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
