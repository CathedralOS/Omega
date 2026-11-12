#!/usr/bin/env python3
"""Emit prior-report slice reconcile tables for part-1."""
from __future__ import annotations

import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import classify_all  # noqa: E402

REPO = Path("/agent/repos/omega")
ART = REPO / ".agents/ledger-bisect-2026-11-11"


def parse_report_clusters() -> list[dict]:
    text = subprocess.check_output(
        [
            "git",
            "show",
            "origin/cursor/ledger-regressions-report-d9c6:wiki/drafts/audits/ledger_regressions_2026-11-11.md",
        ],
        cwd=REPO,
        text=True,
    )
    clusters: list[dict] = []
    cur: dict | None = None
    ins = False
    for line in text.splitlines():
        if line.startswith("## Clusters still broken"):
            ins = True
            continue
        if ins and line.startswith("## "):
            break
        if not ins:
            continue
        m = re.match(
            r"\| (\d+) \| ([^|]+) \| ([^|]+) \| `([^`]+)` \| `([^`]+)` \|", line
        )
        if m:
            if cur:
                clusters.append(cur)
            cur = {
                "rows": int(m.group(1)),
                "witness": m.group(4),
                "members": [],
            }
            continue
        if cur and line.startswith("Members:"):
            cur["members"] = re.findall(r"`([^`]+)`", line)
    if cur:
        clusters.append(cur)
    clusters.sort(key=lambda c: (-c["rows"], c["witness"]))
    return clusters


def main() -> int:
    N, k = 40, 1
    clusters = parse_report_clusters()
    reg = [
        l.strip()
        for l in (ART / "regressed_fixtures.txt").read_text().splitlines()
        if l.strip()
    ]
    fam = {
        f
        for f in reg
        if "foreign_nested_record_backing_" in f or "foreign_retained_" in f
    }
    slice_by_idx: dict[int, set[str]] = {}
    sweep_extra: set[str] = set()
    for i, c in enumerate(clusters):
        if i % N != k:
            continue
        if i == 1:
            slice_by_idx[i] = set(fam)
            sweep_extra = {
                "pass/facts/foreign_nested_record_backing_repeated_child_type",
                "pass/facts/foreign_retained_mutable_borrow_mixed_issuers",
                "pass/facts/foreign_retained_mutable_borrow_reverse_completion",
                "pass/facts/foreign_retained_shared_borrow_enclosing_allocation",
            }
        else:
            ms = set(c["members"]) or {c["witness"]}
            slice_by_idx[i] = ms
    slice_fixtures = sorted({f for s in slice_by_idx.values() for f in s})
    Path("/tmp/slice_fixtures_list.txt").write_text("\n".join(slice_fixtures) + "\n")
    data = classify_all(ART / "pin_observed.txt", Path("/tmp/slice_fixtures_list.txt"))
    broken = set(data["broken_detail"])
    idx_of: dict[str, int] = {}
    for idx, members in slice_by_idx.items():
        for m in members:
            if m not in sweep_extra:
                idx_of[m] = idx
    by = defaultdict(lambda: {"members": 0, "fixed": 0, "broken": 0})
    for f in slice_fixtures:
        if f in sweep_extra:
            continue
        idx = idx_of.get(f, -1)
        by[idx]["members"] += 1
        if f in broken:
            by[idx]["broken"] += 1
        else:
            by[idx]["fixed"] += 1
    print("| prior idx | members | fixed @PIN | broken @PIN |")
    print("| ---: | ---: | ---: | ---: |")
    for idx in sorted(by):
        row = by[idx]
        print(f"| {idx} | {row['members']} | {row['fixed']} | {row['broken']} |")
    tot = {k: sum(by[i][k] for i in by) for k in ("members", "fixed", "broken")}
    print(f"| **total** | **{tot['members']}** | **{tot['fixed']}** | **{tot['broken']}** |")
    print("\n### Idx-1 name-sweep extras (outside slice totals)\n")
    for f in sorted(sweep_extra):
        st = "broken" if f in broken else "fixed"
        print(f"- `{f}` ({st} @PIN)")
    print(
        "\npersistent_retention: prior-report idx **1** (foreign_retained name sweep); "
        "PIN cluster **3** (mutation-while-borrow diagnostic)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
