#!/usr/bin/env python3
"""Collect all fixture members for report slice k of N."""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
REPORT = Path("/tmp/ledger_report.md")
BR = "origin/cursor/ledger-regressions-report-d9c6"


def parse_report_clusters(text: str) -> list[dict]:
    clusters: list[dict] = []
    lines = text.splitlines()
    row_re = re.compile(r"^\| (\d+) \| ([^|]+) \| (.+?) \| `([^`]+)` \|")
    i = 0
    while i < len(lines):
        m = row_re.match(lines[i])
        if m and "Breaking commit" not in lines[i]:
            rows, kind, diag, witness = m.groups()
            listed: list[str] = []
            if i + 1 < len(lines) and lines[i + 1].startswith("Members:"):
                listed = re.findall(r"`([^`]+)`", lines[i + 1])
            clusters.append(
                {
                    "rows": int(rows),
                    "kind": kind.strip(),
                    "diag_sample": diag.strip(),
                    "witness": witness.strip(),
                    "members_listed": listed,
                }
            )
        i += 1
    return clusters


def expand_cluster(c: dict, regressed: list[str]) -> list[str]:
    members = set(c["members_listed"]) | {c["witness"]}
    if c["rows"] >= 10 and len(listed := c["members_listed"]):
        sample = listed[0]
        if "_through_" in sample:
            stem = sample.split("_through_")[0]
            for f in regressed:
                if stem in f:
                    members.add(f)
        if "descriptor_range_exclusive" in " ".join(listed + [c["witness"]]):
            for f in regressed:
                if "descriptor_range_exclusive_loan" in f or "runtime_reassigned_view" in f:
                    members.add(f)
        elif "/" in sample:
            parent = "/".join(sample.split("/")[:-1])
            for f in regressed:
                if f.startswith(parent + "/"):
                    members.add(f)
    return sorted(members)


def main() -> int:
    k, n = int(sys.argv[1]), int(sys.argv[2])
    ranked = sorted(
        parse_report_clusters(REPORT.read_text()),
        key=lambda c: (-c["rows"], c["diag_sample"]),
    )
    regressed = subprocess.check_output(
        ["git", "-C", str(REPO), "show", f"{BR}:wiki/drafts/audits/regressed_fixtures.txt"],
        text=True,
    ).splitlines()
    slice_tables = [c for i, c in enumerate(ranked) if i % n == k]
    out_clusters = []
    all_members: list[str] = []
    for i, c in enumerate(slice_tables):
        members = expand_cluster(c, regressed)
        out_clusters.append({**c, "prior_index": i * n + k, "members": members})
        all_members.extend(members)
    all_members = sorted(set(all_members))
    import json

    Path(f"/tmp/slice_{k}_members.json").write_text(
        json.dumps({"clusters": out_clusters, "members": all_members}, indent=2)
    )
    Path(f"/tmp/slice_{k}_members.txt").write_text("\n".join(all_members) + "\n")
    print(f"fixtures={len(all_members)} prior_tables={len(slice_tables)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
