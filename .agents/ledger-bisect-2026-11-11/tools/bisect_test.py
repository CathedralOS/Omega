#!/usr/bin/env python3
"""Fixed PIN-outcome bisect test (does not read per-revision ledger)."""
from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path

WITNESS = os.environ.get("BISECT_WITNESS", "")
META_PATH = Path(
    f"/tmp/ledger-bisect-witness-{WITNESS.replace('/', '_')}.json"
)
REC_PATH = Path("/tmp/bisect_one_record.txt")


def norm_diag(d: str) -> str:
    d = d.strip()
    d = re.sub(r"`[^`]*`", "`_`", d)
    d = re.sub(r"\b\d+\b", "N", d)
    return d[:160]


def parse_record(text: str, witness: str) -> tuple[str | None, str]:
    line = next((l for l in text.splitlines() if l.startswith(witness + " ")), None)
    if not line:
        return None, ""
    status = line.split()[1]
    diags: list[str] = []
    active = False
    for raw in text.splitlines():
        if raw.startswith(witness + " "):
            active = True
            continue
        if active:
            if raw.startswith("\t"):
                diags.append(raw[1:].strip())
            elif raw.strip():
                break
    first = diags[0] if diags else ""
    return status, first


def main() -> int:
    if not WITNESS or not META_PATH.is_file():
        return 125
    meta = json.loads(META_PATH.read_text())
    text = REC_PATH.read_text() if REC_PATH.exists() else ""
    status, first = parse_record(text, WITNESS)
    if status is None:
        return 125
    expected = meta["expected_status"]
    bad_status = meta["bad_status"]
    if status == expected:
        return 0
    tier = meta["witness"].split("/")[0]
    rank = {"built": 3, "checked": 2, "rejected": 1, "incomplete": 1, "timeout": 0}
    worse_than_expected = rank.get(status, 0) < rank.get(expected, 2)
    if status == bad_status:
        if meta["bad_first_diagnostic"]:
            if first == meta["bad_first_diagnostic"] or norm_diag(first) == meta["bad_diag_norm"]:
                return 1
        return 1
    if worse_than_expected and tier != "fail":
        return 1
    if tier == "fail" and status not in ("rejected", "incomplete") and expected in (
        "rejected",
        "incomplete",
    ):
        return 1
    return 125


if __name__ == "__main__":
    raise SystemExit(main())
