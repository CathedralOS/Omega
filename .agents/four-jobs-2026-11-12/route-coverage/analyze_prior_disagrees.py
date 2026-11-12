#!/usr/bin/env python3
"""One-off: breakdown of pre-partial-fix disagree rows by tier and diagnostic class."""

from __future__ import annotations

import collections
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
TSV = REPO / ".agents/four-jobs-2026-11-12/route-coverage/shard-4.pre-partial.tsv"
HARNESS = REPO / "target-o3/debug/examples/route_one_comparison"
sys.path.insert(0, str(REPO / ".agents/four-jobs-2026-11-12/route-coverage"))
from run_shard_fixture import diagnostic_class, parse_current_diagnostics

rows = []
for line in TSV.read_text().splitlines()[1:]:
    parts = line.split("\t")
    if len(parts) >= 5 and parts[4] == "disagree":
        rows.append(parts)

by_tier = collections.Counter(p[1] for p in rows)
by_combo = collections.Counter((p[2], p[3]) for p in rows)
by_class = collections.Counter()

for fixture, tier, route, current, *_ in rows:
    cmd = [str(HARNESS)]
    if "/fail/" in fixture:
        cmd.append("--every-machine")
    cmd.append(fixture)
    proc = subprocess.run(
        cmd, cwd=REPO, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=300
    )
    diags = parse_current_diagnostics(proc.stdout.splitlines(), fixture)
    if not diags:
        by_class["no-checker-diagnostic"] += 1
    else:
        by_class[diagnostic_class(diags[0])] += 1

print("fixtures", len(rows))
print("by_tier", dict(by_tier))
print("route x current", dict(by_combo))
print("first diagnostic class", by_class.most_common())
