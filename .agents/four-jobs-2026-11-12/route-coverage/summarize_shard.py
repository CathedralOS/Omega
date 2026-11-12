#!/usr/bin/env python3
"""Write shard-S.md summary from shard-S.tsv."""

from __future__ import annotations

import collections
import sys
from pathlib import Path

SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 4
REPO = Path(__file__).resolve().parents[3]
OUT_DIR = REPO / ".agents/four-jobs-2026-11-12/route-coverage"
TSV = OUT_DIR / f"shard-{SHARD}.tsv"
MD = OUT_DIR / f"shard-{SHARD}.md"
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
HARNESS = REPO / "target-o3/debug/examples/route_one_comparison"
PRIOR_BREAKDOWN = Path("/opt/cursor/artifacts/prior_disagree_breakdown.log")


def is_construct_refusal(name: str) -> bool:
    return bool(name) and not name.startswith("bucket:")


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
    judged = sum(1 for r in rows if r["route_verdict"] in {"accepted", "rejected"})
    soundness = [
        r
        for r in rows
        if r["route_verdict"] == "accepted" and r["current_verdict"] == "rejected"
    ]
    route_strict = [
        r
        for r in rows
        if r["route_verdict"] == "rejected" and r["current_verdict"] == "accepted"
    ]
    construct_outside = [
        r
        for r in rows
        if r["route_verdict"] == "outside" and is_construct_refusal(r["first_refusal"])
    ]
    refusal_counts = collections.Counter(r["first_refusal"] for r in construct_outside)

    try:
        import hashlib

        harness_id = hashlib.sha256(HARNESS.read_bytes()).hexdigest()[:12]
    except OSError:
        harness_id = "missing-binary"

    prior_section = ""
    if PRIOR_BREAKDOWN.exists():
        prior_section = (
            "\n## Prior disagree breakdown (pre-partial mapping)\n\n```\n"
            + PRIOR_BREAKDOWN.read_text(encoding="utf-8", errors="replace").strip()
            + "\n```\n"
        )

    lines = [
        f"# Route coverage map — shard {SHARD} of 8",
        "",
        f"Measured commit (PIN): `{PIN}`.",
        f"Harness: `{HARNESS.relative_to(REPO)}` (sha256 prefix `{harness_id}`).",
        "",
        "## Verdict rules (post-partial)",
        "",
        "`current_verdict` is harness `current=` (`accepted` / `rejected` / `stopped`). `stopped` → `error`, not disagree.",
        "",
        "On `fail/` with `--every-machine`, fixture `route_verdict=accepted` only when every root is `accepted` (no sibling `outside` / `no-entry` / …). Mixed `accepted` + skipped roots → `partial` (agreement `n-a`).",
        "",
        "## Commands",
        "",
        "```bash",
        "CARGO_PROFILE_DEV_OPT_LEVEL=3 CARGO_TARGET_DIR=$PWD/target-o3 \\",
        "  mbx build -p compiler --example route_one_comparison < /dev/null",
        "nice -n 19 python3 .agents/four-jobs-2026-11-12/route-coverage/reprocess_shard.py 4",
        "```",
        prior_section,
        "## Totals (this shard)",
        "",
        f"| Fixtures | {total} |",
        f"| Judged (accepted ∪ rejected) | {judged} |",
        f"| Partial | {by_route['partial']} |",
        f"| Outside | {by_route['outside']} |",
        f"| No entry | {by_route['no-entry']} |",
        f"| Error | {by_route['error']} |",
        f"| Timeout | {by_route['timeout']} |",
        f"| Agree | {by_agree['agree']} |",
        f"| Disagree | {by_agree['disagree']} |",
        f"| Route✓ ∧ current✗ (after partial) | {len(soundness)} |",
        f"| Route✗ ∧ current✓ | {len(route_strict)} |",
        "",
        "Sanity: "
        f"{by_route['accepted'] + by_route['rejected'] + by_route['partial'] + by_route['outside'] + by_route['no-entry'] + by_route['error'] + by_route['timeout']} "
        f"= {total}",
        "",
        "## Top first-refusal constructs (outside)",
        "",
        "| construct | count |",
        "| --- | ---: |",
    ]
    for refusal, count in refusal_counts.most_common(10):
        lines.append(f"| {refusal} | {count} |")

    soundness_section = [
        "",
        "## Soundness-watch hand checks",
        "",
    ]
    if not soundness:
        soundness_section.append("None after partial mapping.")
    else:
        soundness_section.append(
            "(Coordinator: hand-check the five fixtures listed in the final agent report.)"
        )

    MD.write_text("\n".join(lines + soundness_section) + "\n", encoding="utf-8")
    print(f"wrote {MD}")


if __name__ == "__main__":
    main()
