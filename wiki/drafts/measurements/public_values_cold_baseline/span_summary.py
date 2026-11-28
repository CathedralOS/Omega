"""Summarize wall-clock intervals recorded by trace_phases.py."""

import csv
import sys
from collections import defaultdict

rows = defaultdict(list)
with open(sys.argv[1]) as stream:
    for row in csv.DictReader(stream, delimiter="\t"):
        rows[row["function"]].append((int(row["start_ns"]), int(row["end_ns"])))
for label, spans in rows.items():
    merged = []
    for begin, end in sorted(spans):
        if merged and merged[-1][1] >= begin:
            merged[-1] = (merged[-1][0], max(merged[-1][1], end))
        else:
            merged.append((begin, end))
    print(label, len(spans), sum(end - begin for begin, end in merged) / 1e9, sep="\t")
