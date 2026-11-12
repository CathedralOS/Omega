#!/usr/bin/env python3
"""Write shard-S.md summary from shard-S.tsv."""

from __future__ import annotations

import subprocess
from collections import Counter
from pathlib import Path

SHARD = 1
DIR = Path(__file__).resolve().parent
REPO = DIR.parents[2]
TSV = DIR / f"shard-{SHARD}.tsv"
MD = DIR / f"shard-{SHARD}.md"
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"


def main() -> None:
    rows = []
    if TSV.is_file():
        for line in TSV.read_text().splitlines()[1:]:
            if not line.strip():
                continue
            cols = line.split("\t")
            if len(cols) >= 7:
                rows.append(cols)

    route = Counter(r[2] for r in rows)
    agree = Counter(r[4] for r in rows)
    outside = [r for r in rows if r[2] == "outside"]
    refusal = Counter(r[5] for r in rows if r[2] == "outside" and r[5] != "-")
    disagreements = [r for r in rows if r[4] == "disagree"]
    timeouts = [r for r in rows if r[2] == "timeout"]
    route_accepts = [
        r for r in disagreements if r[3] == "rejected" and r[2] == "accepted"
    ]

    harness_commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=REPO, text=True
    ).strip()

    lines = [
        f"# Route coverage shard {SHARD}",
        "",
        f"PIN: `{PIN}`",
        f"Harness built at: `{harness_commit}`",
        f"Rows completed: {len(rows)} (shard size 2174)",
        "",
        "## Commands",
        "",
        "Build:",
        "```",
        f"cd {REPO}",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target \\",
        "  mbx build -p compiler --example route_one_comparison < /dev/null",
        "```",
        "",
        "Per fixture (`fail/` uses `--every-machine`):",
        "```",
        "timeout 300 target/debug/examples/route_one_comparison \\",
        "  [--every-machine] <fixture>/main.omg < /dev/null",
        "```",
        "",
        "## Totals (partial until shard completes)",
        "",
        f"- route_verdict: {dict(route)}",
        f"- agreement: {dict(agree)}",
        f"- timeouts: {len(timeouts)}",
        f"- disagreements: {len(disagreements)}",
        f"- route accepts, current rejects (soundness watch): {len(route_accepts)}",
        "",
        "## Outside by first refusal (top 20)",
        "",
    ]
    for name, count in refusal.most_common(20):
        examples = [r[0] for r in outside if r[5] == name][:3]
        lines.append(f"- `{name}`: {count} (e.g. {', '.join(examples)})")
    lines.append("")
    if route_accepts:
        lines.append("## Route accepts / current rejects")
        lines.append("")
        for r in route_accepts[:50]:
            lines.append(f"- `{r[0]}`")
        lines.append("")
    if disagreements:
        lines.append("## All disagreements")
        lines.append("")
        for r in disagreements[:100]:
            lines.append(
                f"- `{r[0]}` tier={r[1]} route={r[2]} current={r[3]}"
            )
        if len(disagreements) > 100:
            lines.append(f"- ... and {len(disagreements) - 100} more")
        lines.append("")
    if timeouts:
        lines.append("## Timeouts")
        lines.append("")
        for r in timeouts:
            lines.append(f"- `{r[0]}`")
        lines.append("")

    MD.write_text("\n".join(lines))


if __name__ == "__main__":
    main()
