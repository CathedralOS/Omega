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

HAND_CHECKS = """
## Agreement definition and hand checks

Agreement compares `current_verdict` to `route_verdict` where both are `accepted` or `rejected`, using only the harness `current=` field from `route_one_comparison` (not corpus tier, not `corpus_outcomes.txt`). `outside`, `error`, and `timeout` are `n-a`.

Fixture-level `route_verdict` follows the harness aggregate: any `rejected` root wins over an `accepted` sibling closure; otherwise first `outside` construct is recorded in `first_refusal` (detail after `Unsupported:` etc., not the reason enum alone). Non-construct buckets use the `bucket:*` prefix (`bucket:no-program-entry`, `bucket:typed-frontier-stopped`, …).

**Five disagreements (route accepts, current rejects)** — all show `current=rejected` on every `program` line while some `machine:*` root has `route=accepted` even though another machine carries the defect the current checker names:

1. `full_width_literal_requires_explicit_policy_conversion`: `machine:invalid` is `outside` (`expression cast into a non-exact arithmetic domain`); `machine:exact` is `route=accepted` while `current=rejected`.
2. `immutable_unsigned_shift_underbound_rejected`: `machine:invalid` outside on contract term; `machine:main` `route=accepted`; current diagnostic targets `invalid`'s ensures.
3. `finite_prefix_skipped_fact`: `machine:seven` `accepted`; `machine:convert` outside on `type addr`.
4. `instruction_disposition_saved_subject_reused`: `machine:main` `accepted`; other machines outside on service reach / assembly target.
5. `instruction_effect_skipped_write`: `machine:main` `accepted`; `machine:observe` outside on `machine assembly target`.

These are genuine `disagree` rows, not tier mix-ups; they are soundness-watch candidates for reconcile.

**Five agreements (both reject)**:

1. `exact_integer_cast_unproven`: `machine:narrow` `route=rejected` with `unproved=1` (wins over `main` accepted).
2. `relative_midpoint_zero_divisor`: `machine:midpoint` `route=rejected`.
3. `short_circuit_cast_wrong_path`: `machine:narrow` and `machine:main` both `route=rejected`.
4. `unequal_observations_do_not_compose`: multiple machines `route=rejected` despite `machine:empty` accepted.
5. `unsigned_quotient_operand_overflow_rejected`: `machine:invalid` `route=rejected`.

## Host build budget (`tools/build_slot.md`)

This shard ran **one fixture per process, serially** (`run_shard.py` / `reprocess_shard.py`), with `nice -n 19` and `timeout 300`. That matches test-VM guidance: one heavy compiler job at a time; no parallel fixture workers on this machine.

## `installed_arrival_*` timeouts

No `installed_arrival_*` fixture falls in shard 4 (`i % 8 == 4` over sorted `main.omg` paths). Nothing to re-verify for the ~6 minute arrival family in this shard.
"""


def is_construct_refusal(name: str) -> bool:
    return name and not name.startswith("bucket:")


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
    construct_outside = [
        r for r in rows if r["route_verdict"] == "outside" and is_construct_refusal(r["first_refusal"])
    ]
    bucket_outside = [
        r for r in rows if r["route_verdict"] == "outside" and r["first_refusal"].startswith("bucket:")
    ]
    refusal_counts = collections.Counter(r["first_refusal"] for r in construct_outside)
    disagreements = [r for r in rows if r["agreement"] == "disagree"]
    soundness = [
        r for r in rows if r["route_verdict"] == "accepted" and r["current_verdict"] == "rejected"
    ]
    timeouts = [r for r in rows if r["route_verdict"] == "timeout"]

    try:
        import hashlib

        harness_id = hashlib.sha256(HARNESS.read_bytes()).hexdigest()[:12]
    except OSError:
        harness_id = "missing-binary"

    lines = [
        f"# Route coverage map — shard {SHARD} of 8",
        "",
        f"Measured commit (PIN): `{PIN}`.",
        f"Harness: `{HARNESS.relative_to(REPO)}` (sha256 prefix `{harness_id}`).",
        "",
        "## Commands",
        "",
        "```bash",
        "CARGO_PROFILE_DEV_OPT_LEVEL=3 CARGO_TARGET_DIR=$PWD/target-o3 \\",
        "  mbx build -p compiler --example route_one_comparison < /dev/null",
        "nice -n 19 python3 .agents/four-jobs-2026-11-12/route-coverage/reprocess_shard.py 4  # parser fix",
        "nice -n 19 python3 .agents/four-jobs-2026-11-12/route-coverage/run_shard.py 4",
        "```",
        "",
        "Per fixture:",
        "",
        "```bash",
        "timeout 300 nice -n 19 target-o3/debug/examples/route_one_comparison [--every-machine] <fixture> < /dev/null",
        "```",
        "",
        HAND_CHECKS.strip(),
        "",
        "## Totals (this shard)",
        "",
        f"| Fixtures measured | {total} |",
        f"| Route judged (accepted ∪ rejected) | {judged} |",
        f"| Agree | {by_agree['agree']} |",
        f"| Disagree | {by_agree['disagree']} |",
        f"| Outside (construct refusal) | {len(construct_outside)} |",
        f"| Outside / error (bucket rows) | {len(bucket_outside) + by_route.get('error', 0)} |",
        f"| Timeout | {by_route['timeout']} |",
        "",
        "## Top first-refusal constructs (outside, excluding buckets)",
        "",
        "| construct | count | examples |",
        "| --- | ---: | --- |",
    ]
    for refusal, count in refusal_counts.most_common(10):
        examples = [r["fixture"] for r in construct_outside if r["first_refusal"] == refusal][:3]
        ex = ", ".join(f"`{e}`" for e in examples)
        lines.append(f"| {refusal} | {count} | {ex} |")

    lines.extend(
        [
            "",
            "## Bucket summary (non-construct)",
            "",
        ]
    )
    bucket_counts = collections.Counter(
        r["first_refusal"] for r in rows if r["first_refusal"].startswith("bucket:")
    )
    for bucket, count in bucket_counts.most_common():
        lines.append(f"- `{bucket}`: {count}")

    lines.extend(
        [
            "",
            f"Soundness watch (route accepts, current rejects): **{len(soundness)}** fixtures.",
            "",
            "## Timeouts",
            "",
        ]
    )
    if not timeouts:
        lines.append("None.")
    else:
        for row in timeouts:
            lines.append(f"- `{row['fixture']}` ({row['seconds']}s)")

    MD.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {MD}")
    print("top constructs:", refusal_counts.most_common(10))


if __name__ == "__main__":
    main()
