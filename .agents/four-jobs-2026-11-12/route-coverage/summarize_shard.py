#!/usr/bin/env python3
"""Write shard-S.md summary from shard-S.tsv."""

from __future__ import annotations

import collections
import subprocess
import sys
from pathlib import Path

SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 4
REPO = Path(__file__).resolve().parents[3]
OUT_DIR = REPO / ".agents/four-jobs-2026-11-12/route-coverage"
TSV = OUT_DIR / f"shard-{SHARD}.tsv"
MD = OUT_DIR / f"shard-{SHARD}.md"
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
HARNESS = REPO / "target-o3/debug/examples/route_one_comparison"


def main() -> None:
    rows: list[dict[str, str]] = []
    for line in TSV.read_text(encoding="utf-8").splitlines()[1:]:
        if not line.strip():
            continue
        parts = line.split("\t")
        while len(parts) < 7:
            parts.append("")
        keys = [
            "fixture",
            "tier",
            "route_verdict",
            "current_verdict",
            "agreement",
            "first_refusal",
            "seconds",
        ]
        rows.append(dict(zip(keys, parts)))

    total = len(rows)
    by_route = collections.Counter(r["route_verdict"] for r in rows)
    by_agree = collections.Counter(r["agreement"] for r in rows)
    outside = [r for r in rows if r["route_verdict"] == "outside" and r["first_refusal"]]
    refusal_counts = collections.Counter(r["first_refusal"] for r in outside)
    disagreements = [r for r in rows if r["agreement"] == "disagree"]
    disagreements.sort(
        key=lambda r: (
            0 if r["route_verdict"] == "accepted" and r["current_verdict"] == "rejected" else 1,
            r["fixture"],
        )
    )
    timeouts = [r for r in rows if r["route_verdict"] == "timeout"]

    harness_commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD:" + str(HARNESS.relative_to(REPO))],
        cwd=REPO,
        text=True,
    ).strip() if False else PIN

    try:
        import hashlib

        harness_commit = hashlib.sha256(HARNESS.read_bytes()).hexdigest()[:12]
    except OSError:
        harness_commit = "missing-binary"

    lines = [
        f"# Route coverage map — shard {SHARD} of 8",
        "",
        f"Measured commit (PIN): `{PIN}`.",
        f"Harness binary: `{HARNESS.relative_to(REPO)}` (content sha256 prefix `{harness_commit}`).",
        "",
        "## Commands",
        "",
        "```bash",
        "cd <omega checkout at PIN>",
        "CARGO_PROFILE_DEV_OPT_LEVEL=3 CARGO_TARGET_DIR=$PWD/target-o3 \\",
        "  mbx build -p compiler --example route_one_comparison < /dev/null",
        f"python3 .agents/four-jobs-2026-11-12/route-coverage/run_shard.py {SHARD}",
        "```",
        "",
        "Per fixture (equivalent to the runner):",
        "",
        "```bash",
        "timeout 300 target-o3/debug/examples/route_one_comparison [--every-machine] <fixture> < /dev/null",
        "```",
        "",
        f"Shard selection: 0-based index `i % 8 == {SHARD}` over sorted `git ls-files` corpus `main.omg` paths.",
        "",
        "## Totals (this shard)",
        "",
        f"| Fixtures measured | {total} |",
        f"| Route judged (accepted or rejected) | {by_route['accepted'] + by_route['rejected']} |",
        f"| Agreement | {by_agree['agree']} |",
        f"| Disagreement | {by_agree['disagree']} |",
        f"| Outside | {by_route['outside']} |",
        f"| Timeout | {by_route['timeout']} |",
        f"| Error | {by_route.get('error', 0)} |",
        "",
        "Timeouts are excluded from outside/rejection buckets (shard notes).",
        "",
        "## First-refusal ranking (outside)",
        "",
        "| first refusal | count | example fixtures |",
        "| --- | ---: | --- |",
    ]
    for refusal, count in refusal_counts.most_common(40):
        examples = [r["fixture"] for r in outside if r["first_refusal"] == refusal][:3]
        lines.append(f"| `{refusal}` | {count} | {', '.join(f'`{e}`' for e in examples)} |")

    lines.extend(
        [
            "",
            "## Disagreements",
            "",
        ]
    )
    if not disagreements:
        lines.append("None in this shard.")
    else:
        for row in disagreements:
            lines.append(
                f"- `{row['fixture']}`: current={row['current_verdict']}, "
                f"route={row['route_verdict']}"
            )

    lines.extend(["", "## Timeouts", ""])
    if not timeouts:
        lines.append("None in this shard.")
    else:
        for row in timeouts:
            lines.append(f"- `{row['fixture']}` ({row['seconds']}s)")

    MD.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {MD}")


if __name__ == "__main__":
    main()
