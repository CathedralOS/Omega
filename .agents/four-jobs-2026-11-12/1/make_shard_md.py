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
HAND10 = JOB_DIR / f"shard-{SHARD}-hand-check.md"
HAND5 = JOB_DIR / f"shard-{SHARD}-hand-check-post-recount.md"
TALLY193 = JOB_DIR / "tally-193-breakdown.txt"

HAND10_SUMMARIES = [
    (
        "tests/omega/fail/arithmetic/immutable_unsigned_shift_invalid_count_rejected/main.omg",
        "Compilation `no-entry`; `machine:invalid` outside (contract term); `machine:main` accepted. "
        "Current rejects shift-count on `invalid`. **Now `partial`**, not soundness.",
    ),
    (
        "tests/omega/fail/arithmetic/match_subject_write_changes_operand_version/main.omg",
        "Defect machines outside (mutable parameter); `machine:main` accepted. Current rejects overflow on `invalid`. **Now `partial`.**",
    ),
    (
        "tests/omega/fail/arithmetic/relative_midpoint_replaced_endpoint/main.omg",
        "`machine:midpoint` outside; `machine:main` accepted. Current rejects midpoint overflow. **Now `partial`.**",
    ),
    (
        "tests/omega/fail/asm/atomic_release_disposition_rebound_descriptor/main.omg",
        "Several machines outside; `done` and `bad` accepted. Current rejects ensures on `finish`. **Now `partial`.**",
    ),
    (
        "tests/omega/fail/asm/borrow_read_after_backward_write/main.omg",
        "Defect machines outside; `good`/`bad` accepted. Current rejects borrow conflict on `bad`. **Now `partial`.**",
    ),
    (
        "tests/omega/pass/arithmetic/bitwise_singleton_signed_and_unsigned_exit/main.omg",
        "Single compilation root: route `rejected` (2 unproved obligations), current `accepted`. **Disagree** (route stricter).",
    ),
    (
        "tests/omega/pass/arithmetic/runtime_boolean_divisor_state_exit/main.omg",
        "Compilation route `rejected` (1 unproved), current `accepted`. **Disagree.**",
    ),
    (
        "tests/omega/pass/effects/private_crashes_none_call_disproved_exit/main.omg",
        "Compilation route `rejected`, current `accepted`. **Disagree.**",
    ),
    (
        "tests/omega/pass/facts/bounded_scalar_sums_through_other_state_exit/main.omg",
        "Compilation route `rejected`, current `accepted`. **Disagree.**",
    ),
    (
        "tests/omega/pass/facts/match_join_multi_variant_bound_through_parameter_exit/main.omg",
        "Compilation route `rejected`, current `accepted`. **Disagree.**",
    ),
]


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
    partial = route_c["partial"]
    outside = route_c["outside"]
    timeouts = route_c["timeout"]
    errors = route_c["error"]

    refusal_c = Counter(
        r["refusal"] for r in rows if r["route"] == "outside" and r["refusal"]
    )

    soundness = [
        r for r in rows if r["route"] == "accepted" and r["current"] == "rejected"
    ]
    false_pass = [
        r for r in rows if r["route"] == "rejected" and r["current"] == "accepted"
    ]

    host = subprocess.check_output(["uname", "-a"], text=True).strip()
    harness = REPO / "target/debug/examples/route_one_comparison"

    lines = [
        f"# Route coverage shard {SHARD} (four-jobs 2026-11-12)",
        "",
        f"Measured commit: `{PIN}`",
        "",
        "## Repo scope",
        "",
        "Only `.agents/four-jobs-2026-11-12/` was changed for this shard work.",
        "`tools/build_slot.md` was **not** modified.",
        "",
        "## Host",
        "",
        f"```\n{host}\n```",
        "",
        f"Harness: `{harness}` (`mbx build -p compiler --example route_one_comparison`).",
        "",
        "## Route verdict (`partial` rule)",
        "",
        "On `fail/` + `--every-machine`, **`partial`** when tally would say `accepted` but",
        "the current checker rejects and at least one relevant root was skipped",
        "(`no-entry`, `outside`, or unjudged machine named in diagnostics).",
        "",
        "## Agreement",
        "",
        "Harness `current=` on the same run. `partial`/`outside`/`timeout`/`error` → `n/a`.",
        "",
        "## `route accepts / current rejects`: 26 → 193 → 1",
        "",
        "| Stage | Count | Meaning |",
        "| --- | ---: | --- |",
        "| First report headline | 26 | Under-count; full TSV already had **193** tally-disagrees |",
        "| After refusal-fix TSV | 193 | All **`fail/`**: tally `accepted` with helper roots passing |",
        "| After **`partial`** recount | **1** | True program-level route accept vs current reject |",
        "",
        "### The 193 (tally-disagree) breakdown",
        "",
        "All **193** were **`fail/`** tier. First `current-diagnostic` class:",
        "",
    ]
    if TALLY193.is_file():
        for ln in TALLY193.read_text().splitlines():
            if ln.strip().startswith(("fixtures", "tiers", "diagnostic")) or ln.strip().startswith(
                tuple("0123456789")
            ):
                lines.append(ln if ln.startswith(" ") else f"- {ln}" if ":" in ln else ln)
        lines.append("")

    lines.extend(
        [
            "## Totals",
            "",
            f"- Fixtures: **{len(rows)}**",
            f"- Judged (accepted + rejected): **{judged}**",
            f"- Partial: **{partial}**",
            f"- Outside: **{outside}**",
            f"- Timeout: **{timeouts}**",
            f"- Error: **{errors}**",
            f"- Agree: **{agree_c['agree']}**",
            f"- Disagree: **{agree_c['disagree']}**",
            f"- n/a: **{agree_c['n-a']}**",
            f"- Soundness watch (route accepted, current rejected): **{len(soundness)}**",
            "",
            "### Route verdict counts",
            "",
        ]
    )
    for key in sorted(route_c.keys()):
        lines.append(f"- `{key}`: {route_c[key]}")

    lines.extend(["", "### Top 10 outside first-refusal constructs", ""])
    for refusal, count in refusal_c.most_common(10):
        lines.append(f"- **{refusal}** ({count})")

    lines.extend(["", "## Hand-check: original 10 fixtures", ""])
    for fixture, summary in HAND10_SUMMARIES:
        lines.append(f"1. `{fixture}` — {summary}")

    lines.extend(["", "## Hand-check: post-recount (5 fixtures)", ""])
    if HAND5.is_file():
        lines.append(HAND5.read_text().strip())
    else:
        lines.append("(missing hand-check post-recount file)")

    for r in soundness:
        lines.extend(
            [
                "",
                f"**Soundness watch candidate:** `{r['fixture']}` — route accepted at",
                "compilation with no unjudged machine root; current rejects UEFI",
                "entry/storage-root contract not modeled on the lowered route.",
            ]
        )

    MD.write_text("\n".join(lines) + "\n")
    print(f"wrote {MD}")


if __name__ == "__main__":
    main()
