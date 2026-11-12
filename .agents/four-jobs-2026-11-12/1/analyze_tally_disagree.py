#!/usr/bin/env python3
"""Break down tally-accepted / current-rejected rows (historical 193) by tier and diagnostic class."""

from __future__ import annotations

import csv
import subprocess
import sys
from collections import Counter
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
HARNESS = REPO / "target/debug/examples/route_one_comparison"

# Import diagnostic helpers from runner
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run_shard_fixture import parse_program_lines, first_diagnostic_class, tier  # noqa: E402


def harness_text(fixture: str) -> str:
    cmd = [str(HARNESS)]
    if tier(fixture) == "fail":
        cmd.append("--every-machine")
    cmd.append(fixture)
    proc = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        timeout=300,
        stdin=subprocess.DEVNULL,
        cwd=str(REPO),
    )
    return proc.stdout


def analyze_tsv(path: Path, filter_acc_rej: bool) -> None:
    rows = list(csv.DictReader(path.open(), delimiter="\t"))
    if filter_acc_rej:
        rows = [
            r
            for r in rows
            if r["route_verdict"] == "accepted" and r["current_verdict"] == "rejected"
        ]
    tiers = Counter(r["tier"] for r in rows)
    print("fixtures", len(rows), "tiers", dict(tiers))

    classes: Counter[str] = Counter()
    with ThreadPoolExecutor(max_workers=3) as pool:
        futs = {pool.submit(harness_text, r["fixture"]): r for r in rows}
        for fut in as_completed(futs):
            text = fut.result()
            fixture = futs[fut]["fixture"]
            _p, _f, diags = parse_program_lines(text, fixture)
            classes[first_diagnostic_class(diags)] += 1
    print("diagnostic class (first current-diagnostic):")
    for k, v in classes.most_common():
        print(f"  {v}\t{k}")


if __name__ == "__main__":
    tsv = Path(sys.argv[1])
    analyze_tsv(tsv, filter_acc_rej=True)
