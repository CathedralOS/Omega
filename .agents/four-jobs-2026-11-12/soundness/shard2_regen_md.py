#!/usr/bin/env python3
"""Regenerate shard-2.md hand-check section from existing TSV (fast)."""
from __future__ import annotations

import csv
import json
from collections import Counter, defaultdict
from pathlib import Path

import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from shard2_run2_reprocess import (  # noqa: E402
    BUCKETS,
    CANONICAL_KINDS,
    MD,
    PROGRESS,
    REPO,
    SHARD,
    TSV,
    fixture_list,
    hand_check_section,
    kind_generation_counts,
    kind_zero_reasons,
)

if __name__ == "__main__":
    rows = list(csv.DictReader(TSV.open(), delimiter="\t"))
    bucket_counts = Counter(r["review_bucket"] for r in rows)
    kind_counts = kind_generation_counts(rows)
    by_kind: dict[str, list[dict]] = defaultdict(list)
    for r in rows:
        by_kind[r["mutation_kind"]].append(r)
    prog = json.loads(PROGRESS.read_text())
    genuine = prog.get("stats", {}).get("run2_original_not_accepted", [])
    fixtures_with_rows = {r["fixture"] for r in rows}
    fx_total = len(fixture_list())
    no_mut = max(0, fx_total - len(fixtures_with_rows) - len(genuine))
    vro = [r for r in rows if r["review_bucket"] == "valid-route-outside"]
    vro_break = Counter(r["route_verdict"] for r in vro)
    missing_route = sum(1 for r in rows if not (r.get("route_verdict") or "").strip())
    confirmed = [r for r in rows if r["review_bucket"] == "confirmed-hole"]

    lines = [
        "# Soundness mutation shard 2 (run 2)",
        "",
        "PIN `0835b3aaf803ed97f905be42d94727ff58459263` | branch `work/soundness-mutations-shard-2`",
        "",
        "## Bucket totals",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    for b in BUCKETS:
        lines.append(f"| `{b}` | {bucket_counts.get(b, 0)} |")
    lines.append(f"| **TSV rows** | **{len(rows)}** |")
    lines.append("")
    lines.extend(["## Mutation kind totals (canonical)", ""])
    for k in CANONICAL_KINDS:
        lines.append(
            f"- `{k}`: **{kind_counts[k]}** — {kind_zero_reasons().get(k, '')}"
        )
    lines.append(f"- per-kind sum: **{sum(kind_counts.values())}** (must equal TSV rows)")
    lines.append("")
    lines.extend(
        [
            "## Integrity",
            "",
            f"- Bucket sum equals TSV rows: **{sum(bucket_counts.values()) == len(rows)}**",
            f"- Per-kind sum equals TSV rows: **{sum(kind_counts.values()) == len(rows)}**",
            f"- Missing route verdicts: **{missing_route}**",
            f"- Confirmed holes with empty spec quote: **{sum(1 for r in confirmed if not (r.get('rule_quote') or '').strip())}**",
            "",
            "## Fixture identity",
            "",
            f"`fixtures ({fx_total}) = mutated ({len(fixtures_with_rows)}) + no-mutation ({no_mut}) + original-not-accepted ({len(genuine)}) + failed-run fixtures (0)`",
            "",
            "## valid-route-outside breakdown",
            "",
        ]
    )
    for rv, n in vro_break.most_common():
        lines.append(f"- `route={rv}`: **{n}**")
    lines.append(f"- total: **{len(vro)}**")
    lines.append("")
    if genuine:
        lines.extend(["## Original-not-accepted (genuine)", ""])
        for rec in genuine[:60]:
            lines.append(
                f"- `{rec['fixture']}`: current={rec['current_verdict']} — {str(rec['diagnostic'])[:160]}"
            )
        lines.append("")
    lines.extend(hand_check_section(by_kind))
    MD.write_text("\n".join(lines))
    print("wrote", MD)
