#!/usr/bin/env python3
"""Write shard-S.md summary from shard-S.tsv."""

from __future__ import annotations

import subprocess
import sys
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
JOB_DIR = Path(__file__).resolve().parent
SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 2
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
TSV = JOB_DIR / f"shard-{SHARD}.tsv"
MD = JOB_DIR / f"shard-{SHARD}.md"


def main() -> None:
    rows = []
    for line in TSV.read_text().splitlines()[1:]:
        if not line.strip():
            continue
        parts = line.split("\t")
        if len(parts) >= 7:
            rows.append(
                {
                    "fixture": parts[0],
                    "tier": parts[1],
                    "route": parts[2],
                    "current": parts[3],
                    "agreement": parts[4],
                    "refusal": parts[5],
                    "seconds": parts[6],
                }
            )

    route_c = Counter(r["route"] for r in rows)
    agree_c = Counter(r["agreement"] for r in rows)
    judged = route_c["accepted"] + route_c["rejected"]
    outside = route_c["outside"]
    timeouts = route_c["timeout"]
    errors = route_c["error"]

    refusal_c = Counter(
        r["refusal"] for r in rows if r["route"] == "outside" and r["refusal"]
    )

    disagreements = [
        r for r in rows if r["agreement"] == "disagree"
    ]
    soundness_top = [
        r
        for r in disagreements
        if r["route"] == "accepted" and r["current"] == "rejected"
    ]

    host = subprocess.check_output(["uname", "-a"], text=True).strip()
    harness = REPO / "target/debug/examples/route_one_comparison"

    lines = [
        f"# Route coverage shard {SHARD} (four-jobs 2026-11-12)",
        "",
        f"Measured commit: `{PIN}`",
        "",
        "## Host",
        "",
        f"```\n{host}\n```",
        "",
        "## Harness",
        "",
        "```bash",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target \\",
        "  mbx build -p compiler --example route_one_comparison",
        "```",
        "",
        f"Binary: `{harness}`",
        "",
        "## Commands",
        "",
        "```bash",
        "# Fixture universe (full corpus; this shard = lines with i % 8 == S)",
        "git ls-files 'tests/omega/pass/**/main.omg' 'tests/omega/run/**/main.omg' \\",
        "  'tests/omega/fail/**/main.omg' | LC_ALL=C sort",
        "",
        "# One fixture (fail/ adds --every-machine); timeout 300 per process",
        "timeout 300 target/debug/examples/route_one_comparison <fixture>/main.omg < /dev/null",
        "timeout 300 target/debug/examples/route_one_comparison --every-machine \\",
        "  <fail-fixture>/main.omg < /dev/null",
        "",
        f"python3 .agents/four-jobs-2026-11-12/1/run_shard.py {SHARD} 3",
        "```",
        "",
        "## Shard totals",
        "",
        f"- Fixtures in shard: **{len(rows)}**",
        f"- Judged (accepted + rejected): **{judged}**",
        f"- Outside: **{outside}**",
        f"- Timeout (separate bucket, not counted as outside): **{timeouts}**",
        f"- Error: **{errors}**",
        f"- Agree: **{agree_c['agree']}**",
        f"- Disagree: **{agree_c['disagree']}**",
        f"- Agreement n/a: **{agree_c['n-a']}**",
        "",
        "### Route verdict counts",
        "",
    ]
    for key in sorted(route_c.keys()):
        lines.append(f"- `{key}`: {route_c[key]}")
    lines.extend(["", "### Outside grouped by first refusal (top 30)", ""])
    for refusal, count in refusal_c.most_common(30):
        examples = [
            r["fixture"]
            for r in rows
            if r["route"] == "outside" and r["refusal"] == refusal
        ][:3]
        ex = ", ".join(f"`{e}`" for e in examples)
        lines.append(f"- **{refusal}** ({count}): {ex}")
    lines.extend(["", "### Disagreements (route accepts, current rejects) — soundness watch", ""])
    if soundness_top:
        for r in soundness_top[:50]:
            lines.append(f"- `{r['fixture']}` ({r['seconds']}s)")
    else:
        lines.append("- (none in this shard)")
    lines.extend(["", "### All disagreements", ""])
    for r in disagreements[:100]:
        lines.append(
            f"- `{r['fixture']}` route={r['route']} current={r['current']}"
        )
    if len(disagreements) > 100:
        lines.append(f"- … and {len(disagreements) - 100} more (see TSV)")

    MD.write_text("\n".join(lines) + "\n")
    print(f"wrote {MD}")


if __name__ == "__main__":
    main()
