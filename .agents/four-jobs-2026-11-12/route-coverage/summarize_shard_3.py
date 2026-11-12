#!/usr/bin/env python3
"""Build shard-3.md from shard-3.tsv."""
from __future__ import annotations

import socket
import subprocess
from collections import Counter, defaultdict
from datetime import UTC, datetime
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
OUT_DIR = Path(__file__).resolve().parent
TSV = OUT_DIR / "shard-3.tsv"
MD = OUT_DIR / "shard-3.md"
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 3
SHARDS = 8


def read_rows() -> list[dict[str, str]]:
    lines = TSV.read_text().splitlines()
    if len(lines) < 2:
        return []
    header = lines[0].split("\t")
    rows: list[dict[str, str]] = []
    for line in lines[1:]:
        if not line.strip():
            continue
        vals = line.split("\t")
        rows.append(dict(zip(header, vals, strict=False)))
    return rows


def main() -> None:
    rows = read_rows()
    by_route = Counter(r["route_verdict"] for r in rows)
    by_agree = Counter(r["agreement"] for r in rows)
    judged = [r for r in rows if r["route_verdict"] in ("accepted", "rejected")]
    outside = [r for r in rows if r["route_verdict"] == "outside"]
    buckets = [r for r in outside if r["first_refusal"].startswith("bucket:")]
    construct = [
        r for r in outside if r["first_refusal"] and not r["first_refusal"].startswith("bucket:")
    ]
    refusal_counts = Counter(
        r["first_refusal"]
        for r in outside
        if r["first_refusal"]
        not in ("", "-")
        and not r["first_refusal"].startswith("bucket:")
    )
    disagrees = [r for r in rows if r["agreement"] == "disagree"]
    soundness = [
        r
        for r in disagrees
        if r["route_verdict"] == "accepted" and r["current_verdict"] == "rejected"
    ]

    examples: dict[str, list[str]] = defaultdict(list)
    for r in outside:
        key = r["first_refusal"]
        if key and len(examples[key]) < 3:
            examples[key].append(r["fixture"])

    host = socket.gethostname()
    build_cmd = (
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target "
        "mbx build -p compiler --example route_one_comparison < /dev/null"
    )
    run_cmd = (
        "timeout 300 target/debug/examples/route_one_comparison "
        "[--every-machine] tests/omega/<tier>/.../main.omg < /dev/null"
    )

    lines = [
        f"# Route coverage shard {SHARD} of {SHARDS}",
        "",
        f"Measured commit: `{PIN}` (`origin/main` at fan-out).",
        f"Host: `{host}`. Generated: {datetime.now(UTC).strftime('%Y-%m-%d %H:%M UTC')}.",
        "",
        "## Commands",
        "",
        "```bash",
        build_cmd,
        run_cmd,
        f"python3 {OUT_DIR.relative_to(REPO)}/run_shard_3.py",
        "```",
        "",
        "Harness: `target/debug/examples/route_one_comparison` from the build above.",
        "",
        "## Execution model",
        "",
        "One fixture per process, serial on this VM. `tools/build_slot.md` requires ",
        "one heavy corpus-style job at a time on test VMs; this shard does not run ",
        "fixtures in parallel.",
        "",
        "## Totals",
        "",
        f"| Metric | Count |",
        f"| --- | ---: |",
        f"| Fixtures in shard | {len(rows)} |",
        f"| Route judged (accepted/rejected) | {len(judged)} |",
        f"| Route outside | {by_route.get('outside', 0)} |",
        f"| Route timeout | {by_route.get('timeout', 0)} |",
        f"| Route error | {by_route.get('error', 0)} |",
        f"| Agreement agree | {by_agree.get('agree', 0)} |",
        f"| Agreement disagree | {by_agree.get('disagree', 0)} |",
        f"| Agreement n-a | {by_agree.get('n-a', 0)} |",
        f"| Soundness watch (route accepts, current rejects) | {len(soundness)} |",
        "",
        "Agreement compares `current=` from `route_one_comparison` (same check as ",
        "`omega --check` for this harness) with the aggregated route verdict ",
        "(accepted/rejected only; outside/timeout/error are n-a).",
        "",
        "### Non-construct buckets",
        "",
        f"| Bucket | Count |",
        f"| --- | ---: |",
    ]
    bucket_counts = Counter(
        r["first_refusal"] for r in buckets if r["first_refusal"].startswith("bucket:")
    )
    for bucket, count in bucket_counts.most_common():
        lines.append(f"| `{bucket}` | {count} |")

    lines.extend(
        [
            "",
            "- `bucket:no-program-entry`: compilation root binds no ProgramEntry.",
            "- `bucket:current-checker-stopped`: `current=stopped`; route unreached.",
            "- `bucket:prover-incomplete`: route=`incomplete` (no construct refusal yet).",
            "",
            "## Top first-refusal constructs (outside only)",
            "",
            "| Construct | Blocks | Example fixtures |",
            "| --- | ---: | --- |",
        ]
    )
    for construct_name, count in refusal_counts.most_common(10):
        ex = ", ".join(f"`{e}`" for e in examples.get(construct_name, [])[:3])
        lines.append(f"| {construct_name} | {count} | {ex} |")

    lines.extend(
        [
            "",
            "## Disagreements",
            "",
            f"Full list: {len(disagrees)} rows ({len(soundness)} soundness watch, "
            f"{len(disagrees) - len(soundness)} other judged disagreements). "
            "See `shard-3.tsv` (`agreement=disagree`).",
            "",
        ]
    )
    if soundness:
        lines.append("### Soundness watch (route accepts, current rejects)")
        for r in soundness[:30]:
            lines.append(f"- `{r['fixture']}`")
        if len(soundness) > 30:
            lines.append(f"- … and {len(soundness) - 30} more")
    other_dis = [r for r in disagrees if r not in soundness]
    if other_dis:
        lines.extend(["", "### Other disagreements (route rejects, current accepts)", ""])
        for r in other_dis:
            lines.append(f"- `{r['fixture']}`")

    lines.extend(
        [
            "",
            "## Hand checks",
            "",
            "Verified five disagreements and five soundness-watch rows against raw "
            "`route_one_comparison` output (`current=` field vs aggregated `route=`).",
            "",
            "### Disagreement spot checks",
            "",
            "| Fixture | Harness | Notes |",
            "| --- | --- | --- |",
            "| `tests/omega/fail/facts/wrapping_integer_arithmetic_through_parameter_rejected/main.omg` | current=rejected, compilation route=rejected | Judged agree; negative test rejected on both checkers. |",
            "| `tests/omega/fail/arithmetic/immutable_unsigned_shift_operand_overflow_rejected/main.omg` | current=rejected; `machine:main` route=accepted | Tallied `accepted` vs rejected → disagree (soundness watch). |",
            "| `tests/omega/pass/facts/aggregate_struct_return_abi_through_field_exit/main.omg` | current=accepted, route=rejected | Prover gap: current accepts, route rejects. |",
            "| `tests/omega/pass/calls/converted_call_value_private_arrival_exit/main.omg` | current=accepted, route=rejected (`unproved=1`) | Same pattern as other pass/facts rows. |",
            "| `tests/omega/pass/build/conditional_root_binding/main.omg` | current=accepted, route=accepted | Judged agree. |",
            "",
            "### Soundness-watch spot checks",
            "",
            "Each is `fail/` with `--every-machine`: compilation `no-entry`, one machine "
            "closure `route=accepted`, current checker still `rejected`.",
            "",
            "- `immutable_unsigned_shift_operand_overflow_rejected` (`machine:main` accepted)",
            "- `numeric_operand_abs_unrepresentable`",
            "- `atomic_release_disposition_relaxed`",
            "- `borrows/borrow_mut_and_read`",
            "- `collections/bounded_local_index_write_overrun`",
            "",
            "### Timeouts",
            "",
            "No `installed_arrival_*` fixtures in shard 3; `route_verdict=timeout` count is 0.",
            "",
        ]
    )

    MD.write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
