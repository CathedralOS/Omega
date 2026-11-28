"""Summarize results.jsonl and compiler phase logs from measure.py."""

import csv
import json
import re
import statistics
import sys
from collections import defaultdict
from pathlib import Path

root = Path(sys.argv[1])
rows = [json.loads(raw) for raw in (root / "results.jsonl").read_text().splitlines()]
timings = defaultdict(list)
for row in rows:
    if row["label"] not in ("cold", "edited") or row["rc"] != 0:
        continue
    group = row["client"].split("-")[0]
    key = (group, row["label"])
    for name in ("seconds", "peak_kib", "bytes"):
        timings[(*key, name)].append(row[name])
    if group == "instrumented":
        lines = (
            (root / row["client"] / (row["label"] + ".stderr")).read_text().splitlines()
        )
        phases = defaultdict(float)
        for line in lines:
            m = re.fullmatch(r"\s*([\d.]+) ms  (.+)", line)
            if m:
                phases[m[2]] += float(m[1])
            m = re.fullmatch(
                r"\s*([\d.]+)\.\.\s*([\d.]+) ms \(\s*([\d.]+) ms\)  timeline / (.+)",
                line,
            )
            if m:
                phases["timeline / " + m[4]] += float(m[3])
                if m[4] == "prepare_targets":
                    prepared = float(m[2])
                if m[4] == "settle_rechecks":
                    phases.setdefault(
                        "post-prepare through first settle start",
                        float(m[1]) - prepared,
                    )
        for name, value in phases.items():
            timings[(*key, name)].append(value)
writer = csv.writer(sys.stdout, delimiter="\t", lineterminator="\n")
writer.writerow(
    ["group", "condition", "metric", "count", "min", "median", "mean", "max"]
)
for (group, label, metric), values in timings.items():
    writer.writerow(
        [
            group,
            label,
            metric,
            len(values),
            *(
                f"{v:.6f}"
                for v in [
                    min(values),
                    statistics.median(values),
                    statistics.mean(values),
                    max(values),
                ]
            ),
        ]
    )
