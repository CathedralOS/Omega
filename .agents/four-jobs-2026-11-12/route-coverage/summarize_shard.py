#!/usr/bin/env python3
"""Print bucket totals from shard TSV."""

from __future__ import annotations

import csv
import sys
from collections import Counter
from pathlib import Path

SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 2
TSV = Path(__file__).resolve().parent / f"shard-{SHARD}.tsv"

rows = list(csv.DictReader(TSV.open(), delimiter="\t"))
by = Counter(r["program_verdict"] for r in rows)
ag = Counter(r["agreement"] for r in rows)
judged = by["accepted"] + by["rejected"]
vacuous_rows = sum(
    1 for r in rows if (r.get("vacuous_accept_roots") or "").strip()
)

sw = [
    r
    for r in rows
    if r["program_verdict"] == "accepted" and r["current_verdict"] == "rejected"
]
print("rows", len(rows))
print("by_verdict", dict(by), "sum", sum(by.values()))
print("judged", judged, "accepted", by["accepted"], "rejected", by["rejected"])
print("agree", dict(ag))
print("rows_with_vacuous", vacuous_rows)
print("soundness_watch", len(sw))
for r in sw:
    print("  ", r["fixture"])
    print("     roots:", r.get("roots", ""))
    print("     vacuous:", r.get("vacuous_accept_roots", ""))
