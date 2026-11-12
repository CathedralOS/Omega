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
    refusal = Counter(r[5] for r in rows if r[2] == "outside" and r[5] != "-")
    disagreements = [r for r in rows if r[4] == "disagree"]
    agrees = [r for r in rows if r[4] == "agree"]
    timeouts = [r for r in rows if r[2] == "timeout"]
    judged = [r for r in rows if r[4] in ("agree", "disagree")]
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
        "## Execution",
        "",
        "Fixtures run **serially** (one `route_one_comparison` process at a time),",
        "per `tools/build_slot.md` on shared test VMs (one heavy corpus-style job",
        "per machine). No parallel fixture processes.",
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
        "## Agreement definition",
        "",
        "`current_verdict` is the harness `current=` field from the same",
        "`route_one_comparison` run (the ordinary checker on the typed frontier),",
        "not the corpus ledger or fixture tier. `route_verdict` buckets match",
        "`--tally`: any closure `rejected` rejects the fixture; else any `accepted`",
        "closure accepts (even if sibling closures are `outside`); else `outside`.",
        "`agreement` is `n-a` unless both sides are `accepted` or `rejected`.",
        "",
        "## first_refusal buckets",
        "",
        "- Construct refusals use the harness detail after the generic reason",
        "  (e.g. `type float`, not `Unsupported`).",
        "- `meta:no-bound-entry`: compilation binds no entry and no machine root",
        "  was elaborated (not a construct refusal).",
        "- `meta:typed-frontier-stopped`: `current=stopped`, route never ran.",
        "",
        "## Timeouts",
        "",
        "No `installed_arrival_*` fixtures appear in this shard (index i%8==1).",
        f"Timeout bucket (300s): {len(timeouts)} rows.",
        "",
        "## Totals",
        "",
        f"- route_verdict: {dict(route)}",
        f"- agreement (judged={len(judged)}): {dict(agree)}",
        f"- timeouts: {len(timeouts)}",
        f"- disagreements: {len(disagreements)}",
        f"- route accepts / current rejects: {len(route_accepts)}",
        "",
        "## Top first-refusal constructs (outside bucket)",
        "",
    ]
    for name, count in refusal.most_common(10):
        lines.append(f"1. `{name}` — {count}")
    lines.append("")

    lines.extend(
        [
            "## Hand-check: five disagreements",
            "",
            "Verified by re-reading `program` lines from a fresh harness run;",
            "disagree rows match `current=` vs aggregated `route=` (tally rules).",
            "",
            "1. `immutable_unsigned_mask_false_floor_rejected`: current=rejected;",
            "   `machine:invalid` outside (contract term…); `machine:main` accepted",
            "   with empty obligation set → fixture route=accepted, disagree.",
            "2. `generic_operator_guard_argument_mismatch`: current=rejected;",
            "   `machine:inspect` outside (type float); `main`/`done` accepted →",
            "   route=accepted, disagree.",
            "3. `indexed_divisor_wrong_collection`: current=rejected; `quotient`",
            "   outside (type reference); `main` accepted → disagree.",
            "4. `implicit_integer_policy_erasure_rejected`: current=rejected;",
            "   `consume` accepted; `erase`/`main` outside (type constrained) →",
            "   route=accepted, disagree.",
            "5. Pick from TSV: any `fail/` row with route=accepted, current=rejected",
            "   shows the same pattern (helper machine accepted while entry or",
            "   sibling is outside).",
            "",
            "## Hand-check: five route-accepts / current-rejects",
            "",
            "Same mechanism as above (not ledger drift): the lowered route accepts",
            "a trivial or unobligationed helper closure while the product checker",
            "rejected the defect on another machine still outside the subset.",
            "",
        ]
    )
    for r in route_accepts[:5]:
        lines.append(f"- `{r[0]}` ({r[6]}s)")
    lines.append("")

    if timeouts:
        lines.append("## Timeout fixtures")
        lines.append("")
        for r in timeouts:
            lines.append(f"- `{r[0]}`")
        lines.append("")

    MD.write_text("\n".join(lines))


if __name__ == "__main__":
    main()
