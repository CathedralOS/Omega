#!/usr/bin/env python3
"""Build part-7 cluster membership from the 2026-11-11 audit report."""
from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
REPORT_REF = "origin/cursor/ledger-regressions-report-d9c6:wiki/drafts/audits/ledger_regressions_2026-11-11.md"
REGRESSED_REF = "origin/cursor/ledger-regressions-report-d9c6:wiki/drafts/audits/regressed_fixtures.txt"
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
PART = 7
MOD = 40


def git_show(ref: str) -> str:
    return subprocess.check_output(["git", "-C", str(REPO), "show", ref], text=True)


def parse_report_clusters(text: str) -> list[dict]:
    start = text.index("## Clusters still broken")
    section = text[start:]
    row_re = re.compile(
        r"^\| (\d+) \| ([^|]+) \| ([^|]+) \| `([^`]+)` \| `([a-f0-9]+)` \|",
        re.M,
    )
    clusters: list[dict] = []
    for m in row_re.finditer(section):
        pos = m.end()
        rest = section[pos : pos + 8000]
        mem_m = re.match(r"\s*Members: (.+?)(?:\n\n|\n\| |\Z)", rest, re.S)
        explicit: list[str] = []
        if mem_m:
            explicit = re.findall(r"`([^`]+)`", mem_m.group(1))
        rows, kind, diag, witness, old_break = m.groups()
        clusters.append(
            {
                "rows": int(rows),
                "kind": kind.strip(),
                "diag_table": diag.strip(),
                "witness": witness,
                "old_break": old_break,
                "members_explicit": explicit,
            }
        )
    return sorted(clusters, key=lambda c: (-c["rows"], c["diag_table"]))


def expand_members(cluster: dict, regressed: set[str]) -> list[str]:
    explicit = list(dict.fromkeys([cluster["witness"], *cluster["members_explicit"]]))
    if len(explicit) >= cluster["rows"]:
        return explicit[: cluster["rows"]]
    witness = cluster["witness"]
    parts = witness.split("/")
  # e.g. fail/facts/source_indexed_type_parameter_through_call_result_rejected
    if len(parts) >= 3:
        family = "/".join(parts[:2]) + "/" + parts[2].split("_through_")[0].split("_wrong_")[0]
    else:
        family = "/".join(parts[:2])
    extra = sorted(f for f in regressed if f.startswith(family) and f not in explicit)
    members = explicit + extra
    return members[: cluster["rows"]] if cluster["rows"] > len(explicit) else members


def main() -> int:
    text = git_show(REPORT_REF)
    regressed = {ln.strip() for ln in git_show(REGRESSED_REF).splitlines() if ln.strip()}
    ranked = parse_report_clusters(text)
    slice_clusters = [
        {**c, "slice_index": i, "members": expand_members(c, regressed)}
        for i, c in enumerate(ranked)
        if i % MOD == PART
    ]
    out = Path(__file__).resolve().parent / "part7_input.json"
    out.write_text(
        json.dumps({"pin": PIN, "part": PART, "clusters": slice_clusters}, indent=2)
    )
    print(f"wrote {out} clusters={len(slice_clusters)}")
    for c in slice_clusters:
        print(
            f"  [{c['slice_index']}] rows={c['rows']} members={len(c['members'])} witness={c['witness']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
