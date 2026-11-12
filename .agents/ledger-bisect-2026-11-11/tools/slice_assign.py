#!/usr/bin/env python3
"""Parse prior report clusters and assign part k of N."""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]


def load_report_clusters(md: str) -> list[dict]:
    rows: list[dict] = []
    cur: dict | None = None
    for line in md.splitlines():
        m = re.match(r"^\| (\d+) \| ([^|]+) \| (.+?) \| `([^`]+)` \| `([0-9a-f]+)`", line)
        if m:
            if cur:
                rows.append(cur)
            cur = {
                "rows": int(m.group(1)),
                "kind": m.group(2).strip(),
                "diagnostic": m.group(3).strip(),
                "witness": m.group(4),
                "old_breaking": m.group(5),
                "members": [],
            }
            continue
        if cur and line.startswith("Members:"):
            cur["members"] = re.findall(r"`([^`]+)`", line.split("…")[0])
    if cur:
        rows.append(cur)
    return rows


def main() -> int:
    k, n = int(sys.argv[1]), int(sys.argv[2])
    md = subprocess.check_output(
        [
            "git",
            "show",
            "origin/cursor/ledger-regressions-report-d9c6:wiki/drafts/audits/ledger_regressions_2026-11-11.md",
        ],
        cwd=REPO,
        text=True,
    )
    clusters = load_report_clusters(md)
    clusters.sort(key=lambda c: (-c["rows"], c["diagnostic"]))
    members_json = json.loads(Path(sys.argv[3]).read_text())["clusters"]
    by_witness = {c["witness"]: c["members"] for c in members_json}
    for i, c in enumerate(clusters):
        c["orig_index"] = i
        if len(c["members"]) < c["rows"] and c["witness"] in by_witness:
            c["members"] = by_witness[c["witness"]]
        elif len(c["members"]) < c["rows"]:
            for mc in members_json:
                if mc["witness"] == c["witness"] or set(c["members"]) <= set(mc["members"]):
                    c["members"] = mc["members"]
                    break
    slice_clusters = [c for c in clusters if c["orig_index"] % n == k]
    out = Path(sys.argv[4])
    out.write_text(json.dumps({"k": k, "n": n, "clusters": slice_clusters}, indent=2))
    total = sum(len(c["members"]) for c in slice_clusters)
    print(f"slice k={k}: {len(slice_clusters)} clusters, {total} member rows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
