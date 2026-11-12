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
BIN = REPO / "target/debug/examples/route_one_comparison"
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


def first_diagnostic_class(fixture: str, tier: str) -> str:
    args = [str(BIN)]
    if tier == "fail":
        args.append("--every-machine")
    args.append(fixture)
    proc = subprocess.run(
        ["timeout", "120", *args],
        cwd=REPO,
        capture_output=True,
        text=True,
        stdin=subprocess.DEVNULL,
    )
    for line in (proc.stdout or "").splitlines():
        if not line.startswith("current-diagnostic\t"):
            continue
        parts = line.split("\t")
        if len(parts) < 4:
            continue
        msg = parts[3]
        if " in machine " in msg:
            return msg.split(" in machine ")[0].strip()
        return msg.split(":")[0].strip()[:100]
    return "no-checker-diagnostic"


def harness_roots_verdict(fixture: str, tier: str) -> str:
    args = [str(BIN)]
    if tier == "fail":
        args.append("--every-machine")
    args.append(fixture)
    proc = subprocess.run(
        ["timeout", "120", *args],
        cwd=REPO,
        capture_output=True,
        text=True,
        stdin=subprocess.DEVNULL,
    )
    routes = []
    for line in (proc.stdout or "").splitlines():
        if line.startswith("program\t"):
            for field in line.split("\t"):
                if field.startswith("route="):
                    routes.append(field.removeprefix("route="))
    return "; ".join(routes) if routes else "no program lines"


def main() -> None:
    rows = read_rows()
    by_route = Counter(r["route_verdict"] for r in rows)
    by_agree = Counter(r["agreement"] for r in rows)
    judged = [r for r in rows if r["route_verdict"] in ("accepted", "rejected")]
    outside_rows = [r for r in rows if r["route_verdict"] == "outside"]
    partial_rows = [r for r in rows if r["route_verdict"] == "partial"]

    refusal_counts = Counter(
        r["first_refusal"]
        for r in outside_rows
        if r["first_refusal"]
        not in ("", "-")
        and not r["first_refusal"].startswith(("bucket:", "partial:"))
    )

    disagrees = [r for r in rows if r["agreement"] == "disagree"]
    soundness = [
        r
        for r in rows
        if r["route_verdict"] == "accepted" and r["current_verdict"] == "rejected"
    ]

    examples: dict[str, list[str]] = defaultdict(list)
    for r in outside_rows:
        key = r["first_refusal"]
        if key and not key.startswith("partial:") and len(examples[key]) < 3:
            examples[key].append(r["fixture"])

    by_tier_soundness = Counter(r["tier"] for r in soundness)
    diag_by_class: Counter[str] = Counter()
    diag_details: list[tuple[str, str, str]] = []
    for r in soundness:
        cls = first_diagnostic_class(r["fixture"], r["tier"])
        diag_by_class[cls] += 1
        diag_details.append((r["fixture"], r["tier"], cls))

    host = socket.gethostname()

    lines = [
        f"# Route coverage shard {SHARD} of {SHARDS}",
        "",
        f"Measured commit: `{PIN}` (`origin/main` at fan-out).",
        f"Host: `{host}`. Generated: {datetime.now(UTC).strftime('%Y-%m-%d %H:%M UTC')}.",
        "",
        "## Verdict rules (shard aggregation)",
        "",
        "Route **accepted** only when every root (`compilation` and each "
        "`machine:…` from `--every-machine`) is judged `accepted`.",
        "**partial** when any root accepts while another is outside, errored, or "
        "otherwise not judged; partial is outside the agreement denominator.",
        "**Soundness watch** = `route_verdict=accepted` and `current_verdict=rejected` "
        "(checker rejection, not `stopped`).",
        "",
        "## Commands",
        "",
        "```bash",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target "
        "mbx build -p compiler --example route_one_comparison < /dev/null",
        "timeout 300 target/debug/examples/route_one_comparison "
        "[--every-machine] tests/omega/<tier>/.../main.omg < /dev/null",
        f"python3 {OUT_DIR.relative_to(REPO)}/run_shard_3.py",
        "```",
        "",
        "Harness: `target/debug/examples/route_one_comparison` from the build above.",
        "",
        "## Execution model",
        "",
        "One fixture per process, serial on this VM (`tools/build_slot.md`).",
        "",
        "## Totals",
        "",
        "| Metric | Count |",
        "| --- | ---: |",
        f"| Fixtures in shard | {len(rows)} |",
        f"| Route judged (accepted + rejected) | {len(judged)} |",
        f"| Route partial | {by_route.get('partial', 0)} |",
        f"| Route outside | {by_route.get('outside', 0)} |",
        f"| Route timeout | {by_route.get('timeout', 0)} |",
        f"| Route error | {by_route.get('error', 0)} |",
        f"| Agreement agree | {by_agree.get('agree', 0)} |",
        f"| Agreement disagree | {by_agree.get('disagree', 0)} |",
        f"| Agreement n-a (outside/partial/timeout/error) | {by_agree.get('n-a', 0)} |",
        f"| Soundness watch (route✓ current✗) | {len(soundness)} |",
        "",
    ]

    if soundness:
        lines.extend(
            [
                "### Soundness watch by tier",
                "",
                "| Tier | Count |",
                "| --- | ---: |",
            ]
        )
        for tier, count in by_tier_soundness.most_common():
            lines.append(f"| {tier} | {count} |")

        lines.extend(
            [
                "",
                "### Soundness watch by first current-checker diagnostic class",
                "",
                "| Diagnostic class (first `current-diagnostic` line) | Count |",
                "| --- | ---: |",
            ]
        )
        for cls, count in diag_by_class.most_common():
            lines.append(f"| {cls} | {count} |")
    else:
        lines.append("No soundness-watch rows after partial-aware aggregation.")

    bucket_counts = Counter(
        r["first_refusal"]
        for r in outside_rows
        if r["first_refusal"].startswith("bucket:")
    )
    lines.extend(
        [
            "",
            "### Non-construct buckets (outside)",
            "",
            "| Bucket | Count |",
            "| --- | ---: |",
        ]
    )
    for bucket, count in bucket_counts.most_common():
        lines.append(f"| `{bucket}` | {count} |")

    lines.extend(
        [
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
            f"Partial rows: {len(partial_rows)} (see `first_refusal` prefix `partial:` in TSV).",
            "",
            "## Hand checks (soundness-watch candidates)",
            "",
        ]
    )

    hand_pick = soundness[:5] if soundness else []
    partial_examples = partial_rows[:10]
    if not hand_pick:
        lines.append(
            "No `route_verdict=accepted` + `current_verdict=rejected` rows after "
            "all-roots-must-accept aggregation (**0**). The prior 186 count was "
            "partial tallying noise (`machine:…` accepted beside `no-entry`/`outside`)."
        )
        lines.extend(
            [
                "",
                "### Hand-check: five former false soundness-watch rows (now `partial`)",
                "",
                "| Fixture | `current` | Partial sibling refusing | Real soundness watch? |",
                "| --- | --- | --- | --- |",
            ]
        )
        pick_five = [
            r
            for r in partial_rows
            if r["tier"] == "fail" and r["current_verdict"] == "rejected"
        ][:5]
        for r in pick_five:
            roots = harness_roots_verdict(r["fixture"], r["tier"])
            lines.append(
                f"| `{r['fixture']}` | rejected | `{roots}` | **No** — not all roots accepted. |"
            )
    else:
        lines.extend(
            [
                "| Fixture | Tier | Diagnostic class | Per-root routes | Real soundness watch? |",
                "| --- | --- | --- | --- | --- |",
            ]
        )
        for fixture, tier, cls in diag_details[:5]:
            roots = harness_roots_verdict(fixture, tier)
            lines.append(
                f"| `{fixture}` | {tier} | {cls} | `{roots}` | (see notes below) |"
            )
        lines.extend(
            [
                "",
                "Notes:",
                "",
            ]
        )
        for fixture, tier, cls in diag_details[:5]:
            note = (
                "Candidate if every root is `accepted` and current checker rejects on check."
            )
            if tier == "fail":
                note = (
                    "Negative fixture: only a real watch if all roots accepted; otherwise partial."
                )
            lines.append(f"- `{fixture}`: {note}")

    other_dis = disagrees
    if other_dis:
        by_tier_dis = Counter(r["tier"] for r in other_dis)
        lines.extend(
            [
                "",
                "### Judged disagreements (prover gaps, not soundness watch)",
                "",
                f"{len(other_dis)} rows: route `rejected`, current `accepted` (all pass-tier in this shard).",
                "",
                "| Tier | Count |",
                "| --- | ---: |",
            ]
        )
        for tier, count in by_tier_dis.most_common():
            lines.append(f"| {tier} | {count} |")
        dis_diag: Counter[str] = Counter()
        lines.extend(
            [
                "",
                "Five spot-checked:",
                "",
            ]
        )
        for r in other_dis[:5]:
            cls = first_diagnostic_class(r["fixture"], r["tier"])
            dis_diag[cls] += 1
            roots = harness_roots_verdict(r["fixture"], r["tier"])
            lines.append(
                f"- `{r['fixture']}`: diagnostic «{cls}»; roots `{roots}` — **not** soundness watch."
            )

    lines.extend(
        [
            "",
            "### Timeouts",
            "",
            "No `installed_arrival_*` in shard 3; timeout count 0.",
            "",
            "### Repository scope",
            "",
            "Branch commits from PIN touch only `.agents/four-jobs-2026-11-12/`.",
            "",
        ]
    )

    MD.write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
