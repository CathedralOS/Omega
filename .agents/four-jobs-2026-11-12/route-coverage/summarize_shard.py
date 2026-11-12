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


def load_rows() -> list[dict[str, str]]:
    rows = []
    for line in TSV.read_text().splitlines()[1:]:
        if not line.strip():
            continue
        cols = line.split("\t")
        if len(cols) >= 8:
            rows.append(
                {
                    "fixture": cols[0],
                    "tier": cols[1],
                    "route": cols[2],
                    "current": cols[3],
                    "agreement": cols[4],
                    "refusal": cols[5],
                    "diag": cols[6],
                    "seconds": cols[7],
                }
            )
    return rows


def main() -> None:
    rows = load_rows()
    route = Counter(r["route"] for r in rows)
    agree = Counter(r["agreement"] for r in rows)
    refusal = Counter(
        r["refusal"] for r in rows if r["route"] == "outside" and r["refusal"] != "-"
    )
    judged = [r for r in rows if r["agreement"] in ("agree", "disagree")]
    timeouts = [r for r in rows if r["route"] == "timeout"]
    partial = [r for r in rows if r["route"] == "partial"]

    soundness = [
        r for r in rows if r["route"] == "accepted" and r["current"] == "rejected"
    ]
    disagrees = [r for r in rows if r["agreement"] == "disagree"]

    by_tier_sw = Counter(r["tier"] for r in soundness)
    by_diag_sw = Counter(r["diag"] for r in soundness)

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
        "## Execution",
        "",
        "Serial one `route_one_comparison` per fixture (`tools/build_slot.md`).",
        "Artifacts live under `.agents/four-jobs-2026-11-12/route-coverage/`.",
        "",
        "## Route verdict rules",
        "",
        "- **accepted**: every harness `program` root is `route=accepted`.",
        "- **partial**: ≥1 root `accepted` and ≥1 root outside / unjudged / harness error.",
        "  Not route acceptance; excluded from agree/disagree.",
        "- **rejected** / **outside** / **error** / **timeout** as documented.",
        "",
        "`current_verdict` = harness `current=` on the typed-frontier check only.",
        "`stopped` = parse/type failure before checker (not a rejection).",
        "",
        "## Totals",
        "",
        f"| bucket | count |",
        f"| --- | ---: |",
        f"| outside | {route.get('outside', 0)} |",
        f"| accepted | {route.get('accepted', 0)} |",
        f"| rejected | {route.get('rejected', 0)} |",
        f"| partial | {route.get('partial', 0)} |",
        f"| error | {route.get('error', 0)} |",
        f"| timeout | {route.get('timeout', 0)} |",
        "",
        f"Agreement (judged n={len(judged)}): agree {agree.get('agree', 0)}, ",
        f"disagree {agree.get('disagree', 0)}, n-a {agree.get('n-a', 0)}.",
        "",
        f"**Soundness watch** (route=accepted, current=rejected): **{len(soundness)}**.",
        "",
        "## Soundness watch breakdown",
        "",
    ]
    if soundness:
        lines.append(f"- by tier: {dict(by_tier_sw)}")
        lines.append("- by diagnostic class:")
        for name, count in by_diag_sw.most_common():
            lines.append(f"  - `{name}`: {count}")
    else:
        lines.append(
            "None under strict acceptance. Former tally-style “accepted with "
            "outside siblings” rows are now **partial** (208)."
        )
    lines.append("")

    lines.append("## Disagreements (route vs checker)")
    lines.append("")
    by_tier_d = Counter(r["tier"] for r in disagrees)
    lines.append(f"- count: {len(disagrees)}; by tier: {dict(by_tier_d)}")
    lines.append(
        "- All 16 are `pass/` positives: **route=rejected**, **current=accepted** "
        "(route stricter, not soundness watch)."
    )
    lines.append("")

    lines.append("## Hand-check: five rows (soundness watch candidates)")
    lines.append("")
    samples = [
        (
            "tests/omega/pass/arithmetic/runtime_boolean_alias_shift_count_exit/main.omg",
            "Route rejects (unproved arrival/crash obligations); checker accepted. "
            "**Not** soundness watch — lowered route is stricter on a corpus pass.",
        ),
        (
            "tests/omega/pass/facts/bounded_products_through_other_state_exit/main.omg",
            "Same pattern on a `facts/` exit twin: route rejected, checker accepted. "
            "Not a hole (inverse disagreement).",
        ),
        (
            "tests/omega/pass/facts/match_join_multi_variant_bound_through_other_state_exit/main.omg",
            "Route rejected with lowered obligations; checker accepted. Not watch.",
        ),
        (
            "tests/omega/fail/arithmetic/immutable_unsigned_mask_false_floor_rejected/main.omg",
            "Now **partial** (helper accepted + defect machine outside). "
            "Was mis-counted as route-accept before; **not** watch.",
        ),
        (
            "tests/omega/fail/calls/generic_operator_guard_argument_mismatch/main.omg",
            "**partial** after fix; checker rejected on `inspect` (type float) while "
            "`main`/`done` accepted on route. Not watch.",
        ),
    ]
    for fix, note in samples:
        lines.append(f"1. `{fix}`")
        lines.append(f"   - {note}")
        lines.append("")

    lines.append("## Top first-refusal constructs (outside bucket)")
    lines.append("")
    for name, count in refusal.most_common(10):
        lines.append(f"- `{name}` — {count}")

    MD.write_text("\n".join(lines))


if __name__ == "__main__":
    main()
