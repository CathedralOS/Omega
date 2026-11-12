#!/usr/bin/env python3
"""Slice prior markdown report clusters (226) by index % N == k; expand members via old diagnostics."""
from __future__ import annotations

import json
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
TOOLS = REPO / ".agents/ledger-bisect-2026-11-11/tools"
sys.path.insert(0, str(TOOLS))
from classify_corpus import classify_all, norm_diag, parse_records  # noqa: E402

REPORT = Path("/tmp/ledger_report.md")
BR = "origin/cursor/ledger-regressions-report-d9c6"


def git_show(path: str) -> str:
    return subprocess.check_output(["git", "-C", str(REPO), "show", f"{BR}:{path}"], text=True)


def parse_report_clusters(text: str) -> list[dict]:
    lines = text.splitlines()
    clusters: list[dict] = []
    i = 0
    row_re = re.compile(r"^\| (\d+) \| ([^|]+) \| (.+?) \| `([^`]+)` \|")
    while i < len(lines):
        m = row_re.match(lines[i])
        if m and "Breaking commit" not in lines[i]:
            rows, kind, diag, witness = m.groups()
            members_line = ""
            if i + 1 < len(lines) and lines[i + 1].startswith("Members:"):
                members_line = lines[i + 1]
            listed = re.findall(r"`([^`]+)`", members_line)
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


def main() -> int:
    k, n = int(sys.argv[1]), int(sys.argv[2])
    report_clusters = parse_report_clusters(REPORT.read_text())
    # Sort like brief: row count desc, tie first diagnostic LC_ALL=C
    ranked = sorted(report_clusters, key=lambda c: (-c["rows"], c["diag_sample"]))
    assert len(ranked) >= 200, len(ranked)

    merged = Path("/tmp/prior_observed_merged.txt")
    parts = [git_show(f"wiki/drafts/audits/corpus/batch_{i}.txt") for i in range(8)]
    parts.append(git_show("wiki/drafts/audits/corpus/host_settled.txt"))
    merged.write_text("\n".join(parts))
    regressed = Path("/tmp/regressed_fixtures.txt")
    regressed.write_text(git_show("wiki/drafts/audits/regressed_fixtures.txt"))
    data = classify_all(merged, regressed)
    fixture_diag: dict[str, str] = {f: d["diag_norm"] for f, d in data["broken_detail"].items()}

    slice_prior = [c for i, c in enumerate(ranked) if i % n == k]
    expanded = []
    for c in slice_prior:
        witness = c["witness"]
        wnorm = fixture_diag.get(witness)
        if wnorm is None:
            # witness fixed since report; use diag_sample prefix match
            members = c["members_listed"]
        else:
            members = sorted(
                f for f, dn in fixture_diag.items() if dn == wnorm and f in data["broken_detail"]
            )
        expanded.append({**c, "members": members, "diag_norm": wnorm or ""})
    members_all = sorted({m for c in expanded for m in c["members"]})
    out = {"k": k, "n": n, "prior_cluster_count": len(ranked), "slice_prior_tables": len(slice_prior), "clusters": expanded}
    Path(f"/tmp/slice_{k}_prior.json").write_text(json.dumps(out, indent=2))
    Path(f"/tmp/slice_{k}_members.txt").write_text("\n".join(members_all) + "\n")
    print(f"report_clusters={len(ranked)} slice_tables={len(slice_prior)} members={len(members_all)}")
    for j, c in enumerate(expanded):
        print(f"  prior_idx={j*n+k} rows={c['rows']} members={len(c['members'])} witness={c['witness']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
