#!/usr/bin/env python3
"""Regenerate shard-S.md summary from shard-S.tsv."""

from __future__ import annotations

import collections
import sys
from datetime import UTC, datetime
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 5
OUT_DIR = REPO / ".agents/four-jobs-2026-11-12/route-coverage"
TSV = OUT_DIR / f"shard-{SHARD}.tsv"
MD = OUT_DIR / f"shard-{SHARD}.md"
HARNESS = REPO / "target/debug/examples/route_one_comparison"


def main() -> None:
    rows: list[dict[str, str]] = []
    if TSV.is_file():
        lines = TSV.read_text().splitlines()
        header = lines[0].split("\t") if lines else []
        for line in lines[1:]:
            if not line.strip():
                continue
            vals = line.split("\t")
            rows.append(dict(zip(header, vals, strict=False)))

    total = len(rows)
    by_route = collections.Counter(r.get("route_verdict", "") for r in rows)
    by_agree = collections.Counter(r.get("agreement", "") for r in rows)
    outside_refusals = collections.Counter(
        r.get("first_refusal", "") for r in rows if r.get("route_verdict") == "outside"
    )
    timeouts = [r for r in rows if r.get("route_verdict") == "timeout"]
    disagreements = [r for r in rows if r.get("agreement") == "disagree"]
    soundness = [
        r
        for r in disagreements
        if r.get("route_verdict") == "accepted" and r.get("current_verdict") == "rejected"
    ]

    top_refusals = outside_refusals.most_common(25)
    md = [
        f"# Route coverage shard {SHARD} of 8",
        "",
        f"- Measured commit (PIN): `{PIN}`",
        f"- Host: Linux x86_64 cloud agent",
        f"- Harness: `{HARNESS.relative_to(REPO)}`",
        f"- Updated: {datetime.now(UTC).strftime('%Y-%m-%dT%H:%MZ')}",
        "",
        "## Commands",
        "",
        "```bash",
        f"git checkout {PIN}",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target \\",
        "  mbx build -p compiler --example route_one_comparison",
        f"python3 {OUT_DIR.relative_to(REPO)}/run_shard.py {SHARD}",
        "```",
        "",
        "Per fixture (fail tier uses `--every-machine`):",
        "",
        "```bash",
        "timeout 300 target/debug/examples/route_one_comparison <fixture>/main.omg < /dev/null",
        "timeout 300 target/debug/examples/route_one_comparison --every-machine <fixture>/main.omg < /dev/null",
        "```",
        "",
        "## Totals",
        "",
        f"| Metric | Count |",
        f"| --- | ---: |",
        f"| Fixtures in shard | {total} |",
        f"| Route accepted | {by_route.get('accepted', 0)} |",
        f"| Route rejected | {by_route.get('rejected', 0)} |",
        f"| Route outside | {by_route.get('outside', 0)} |",
        f"| Route timeout | {by_route.get('timeout', 0)} |",
        f"| Route error | {by_route.get('error', 0)} |",
        f"| Agreement agree | {by_agree.get('agree', 0)} |",
        f"| Agreement disagree | {by_agree.get('disagree', 0)} |",
        f"| Agreement n/a | {by_agree.get('n-a', 0)} |",
        "",
        "## Top first-refusal constructs (outside)",
        "",
        "| Construct | Fixtures | Examples |",
        "| --- | ---: | --- |",
    ]
    for construct, count in top_refusals[:20]:
        if not construct:
            construct = "(empty)"
        examples = [
            r["fixture"]
            for r in rows
            if r.get("route_verdict") == "outside" and r.get("first_refusal", "") == construct
        ][:3]
        ex = ", ".join(f"`{e}`" for e in examples)
        md.append(f"| {construct} | {count} | {ex} |")

    md.extend(
        [
            "",
            "## Disagreements (route accepts, current rejects) — soundness candidates",
            "",
        ]
    )
    if soundness:
        for r in soundness[:200]:
            md.append(f"- `{r['fixture']}` ({r.get('seconds', '?')}s)")
    else:
        md.append("_None in this shard so far._")

    md.extend(["", "## All disagreements", ""])
    if disagreements:
        for r in disagreements[:500]:
            md.append(
                f"- `{r['fixture']}` route={r.get('route_verdict')} current={r.get('current_verdict')}"
            )
    else:
        md.append("_None._")

    md.extend(["", "## Timeouts", ""])
    if timeouts:
        for r in timeouts:
            md.append(f"- `{r['fixture']}` ({r.get('seconds', '?')}s)")
    else:
        md.append("_None._")

    MD.write_text("\n".join(md) + "\n")
    print(f"wrote {MD} ({total} rows)")


if __name__ == "__main__":
    main()
