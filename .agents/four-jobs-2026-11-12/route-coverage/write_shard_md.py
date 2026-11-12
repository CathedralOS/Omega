#!/usr/bin/env python3
"""Regenerate shard-S.md summary from shard-S.tsv."""

from __future__ import annotations

import collections
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 5
OUT_DIR = REPO / ".agents/four-jobs-2026-11-12/route-coverage"
TSV = OUT_DIR / f"shard-{SHARD}.tsv"
MD = OUT_DIR / f"shard-{SHARD}.md"
HARNESS = REPO / "target/debug/examples/route_one_comparison"
NON_CONSTRUCT_BUCKETS = frozenset({"no_program_entry", "current_stopped_before_typed_trees"})


def accepted_roots_from_row(roots: str) -> list[str]:
    names: list[str] = []
    for part in roots.split(";"):
        if not part or ":" not in part:
            continue
        name, verdict = part.rsplit(":", 1)
        if verdict == "accepted":
            names.append(name)
    return names


def vacuous_label(row: dict[str, str]) -> str:
    vacuous = {v for v in row.get("vacuous_accept_roots", "").split(";") if v}
    accepted = accepted_roots_from_row(row.get("roots", ""))
    if not accepted:
        return "n/a"
    if all(r in vacuous for r in accepted):
        return "vacuous"
    if vacuous:
        return "mixed"
    return "non-vacuous"


def harness_output(fixture: str) -> str:
    cmd = [str(HARNESS), "--every-machine", fixture]
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=120,
            stdin=subprocess.DEVNULL,
            cwd=REPO,
        )
        return proc.stdout
    except subprocess.TimeoutExpired:
        return ""


def first_current_diagnostic(fixture: str) -> tuple[str, str]:
    """Return (diagnostic_class, message) from first current-diagnostic line."""
    raw = harness_output(fixture)
    for line in raw.splitlines():
        if not line.startswith("current-diagnostic\t"):
            continue
        parts = line.split("\t")
        message = parts[-1] if len(parts) >= 4 else ""
        lowered = message.lower()
        if "lifetime" in lowered or "undeclared lifetime" in lowered:
            clazz = "lifetime / generic parameter"
        elif "program entry" in lowered or "hosted_program" in fixture or "entry" in lowered[:40]:
            clazz = "program entry / build"
        elif lowered.startswith("data ") or " field `" in message:
            clazz = "data declaration"
        elif "operator" in lowered or "overload" in lowered:
            clazz = "operator / dispatch"
        elif "provider" in lowered or "requirement" in lowered:
            clazz = "provider / requirement"
        elif "domain" in lowered or "alias" in lowered:
            clazz = "domain / alias"
        elif "comptime" in lowered or "const" in fixture and "comptime" in fixture:
            clazz = "comptime / constexpr"
        elif "module" in lowered or "duplicate" in lowered:
            clazz = "module / declaration"
        elif "formation" in lowered or "requires" in lowered:
            clazz = "dependent data / contract"
        else:
            head = message.split(":", 1)[0].strip() if message else "unknown"
            clazz = head[:80] if head else "unknown"
        return clazz, message
    return "(no current-diagnostic line)", ""


def soundness_watch_verdict(fixture: str, route: str, current: str, message: str) -> str:
    if route != "accepted" or current != "rejected":
        return "n/a"
    raw = harness_output(fixture)
    if "program entry" in message.lower() or "build/" in fixture:
        return "yes — route does not judge program-entry/build rules; checker rejection is typed-frontier"
    if any(
        token in message.lower()
        for token in ("lifetime", "data `", "operator", "domain", "provider", "formation", "duplicate")
    ):
        return "yes — checker typed-frontier diagnostic; route accepted an elaborated machine outside that rule class"
    return "review — confirm defect site is not covered by accepted machine root"


def hand_check_notes(fixture: str) -> list[str]:
    raw = harness_output(fixture)
    lines = []
    for line in raw.splitlines():
        if line.startswith("program\t") or line.startswith("current-diagnostic\t"):
            lines.append(line[:500])
    if not lines:
        lines.append("(no program / diagnostic lines in harness output)")
    return lines[:8]


def main() -> None:
    rows: list[dict[str, str]] = []
    if TSV.is_file():
        lines = TSV.read_text().splitlines()
        header = lines[0].split("\t") if lines else []
        for line in lines[1:]:
            if not line.strip():
                continue
            vals = line.split("\t")
            rows.append(dict(zip(header, vals, strict=False)))

    total = len(rows)
    by_route = collections.Counter(r.get("route_verdict", "") for r in rows)
    by_agree = collections.Counter(r.get("agreement", "") for r in rows)
    partial_refusals = collections.Counter(
        r.get("first_refusal", "") for r in rows if r.get("route_verdict") == "partial"
    )
    outside_refusals = collections.Counter(
        r.get("first_refusal", "") for r in rows if r.get("route_verdict") == "outside"
    )
    construct_outside = {
        k: v for k, v in outside_refusals.items() if k not in NON_CONSTRUCT_BUCKETS
    }
    construct_partial = {
        k: v
        for k, v in partial_refusals.items()
        if k not in NON_CONSTRUCT_BUCKETS
    }
    bucket_counts: collections.Counter[str] = collections.Counter()
    for r in rows:
        if r.get("route_verdict") not in ("outside", "partial"):
            continue
        b = r.get("first_refusal", "")
        if b in NON_CONSTRUCT_BUCKETS:
            bucket_counts[b] += 1
    judged = by_route.get("accepted", 0) + by_route.get("rejected", 0)
    bucket_sum = sum(by_route.values())
    agree_disagree = by_agree.get("agree", 0) + by_agree.get("disagree", 0)
    timeouts = [r for r in rows if r.get("route_verdict") == "timeout"]
    disagreements = [r for r in rows if r.get("agreement") == "disagree"]
    soundness = [
        r
        for r in rows
        if r.get("route_verdict") == "accepted" and r.get("current_verdict") == "rejected"
    ]

    top_outside = sorted(construct_outside.items(), key=lambda x: (-x[1], x[0]))[:15]
    top_partial = sorted(construct_partial.items(), key=lambda x: (-x[1], x[0]))[:15]
    md = [
        f"# Route coverage shard {SHARD} of 8",
        "",
        f"- Measured commit (PIN): `{PIN}`",
        f"- Host: Linux x86_64 cloud agent",
        f"- Harness: `{HARNESS.relative_to(REPO)}`",
        f"- Updated: {datetime.now(UTC).strftime('%Y-%m-%dT%H:%MZ')}",
        "",
        "## Execution model",
        "",
        "One `route_one_comparison` process per fixture, serial on this VM "
        "(per `tools/build_slot.md`: one heavy corpus-style job at a time on shared test hosts). "
        "`timeout 300` per fixture; wall seconds recorded in the TSV.",
        "",
        "### Non-construct `first_refusal` buckets",
        "",
        "| Bucket | Meaning |",
        "| --- | --- |",
        "| `no_program_entry` | Fixture binds no program entry; compilation root is `route=no-entry` and was not elaborated as a module. |",
        "| `current_stopped_before_typed_trees` | Current checker `current=stopped`; route `unreached` (no typed trees). |",
        "",
        "Construct refusals use the payload after the harness refusal label "
        "(e.g. `Unsupported: parameter mutable parameter` → `parameter mutable parameter`, "
        "matching `structure_elaboration::refuse` detail text).",
        "",
        "### Agreement definition",
        "",
        "Compare `route_verdict` (`accepted`/`rejected` only) with `current_verdict` "
        "from the same harness `program` line (`current=accepted|rejected|stopped`). "
        "`outside`, `partial`, `timeout`, and `error` → `n-a` (excluded from the "
        "agreement denominator). All tiers use `--every-machine`. Aggregate over "
        "**every** harness `program` root (compilation and each `machine:`). "
        "Any root `rejected` → fixture `rejected`; all roots `accepted` → "
        "`accepted`; `accepted` mixed with `outside` / `no-entry` / other "
        "unjudged and no `rejected` → `partial`. TSV `roots` lists "
        "`root:verdict` pairs; `vacuous_accept_roots` lists accepted roots with "
        "`obligations=0` and `judged=0`.",
        "",
        "## Commands",
        "",
        "```bash",
        f"git checkout {PIN}",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target \\",
        "  mbx build -p compiler --example route_one_comparison",
        f"python3 {OUT_DIR.relative_to(REPO)}/run_shard.py {SHARD}",
        f"python3 {OUT_DIR.relative_to(REPO)}/run_shard.py {SHARD} --repair  # re-parse fixes",
        "```",
        "",
        "Per fixture (all tiers):",
        "",
        "```bash",
        "timeout 300 target/debug/examples/route_one_comparison --every-machine <fixture>/main.omg < /dev/null",
        "```",
        "",
        "## Totals",
        "",
        f"| Metric | Count |",
        f"| --- | ---: |",
        f"| Fixtures in shard | {total} |",
        f"| Route judged (accepted + rejected) | {judged} |",
        f"| Route accepted | {by_route.get('accepted', 0)} |",
        f"| Route rejected | {by_route.get('rejected', 0)} |",
        f"| Route partial | {by_route.get('partial', 0)} |",
        f"| Route outside | {by_route.get('outside', 0)} |",
        f"| Route timeout | {by_route.get('timeout', 0)} |",
        f"| Route error | {by_route.get('error', 0)} |",
        f"| Agreement agree | {by_agree.get('agree', 0)} |",
        f"| Agreement disagree | {by_agree.get('disagree', 0)} |",
        f"| Agreement n/a | {by_agree.get('n-a', 0)} |",
        f"| Sanity: agree+disagree = judged | {agree_disagree} = {judged} |",
        f"| Sanity: verdict buckets sum | {bucket_sum} = {total} |",
        "",
        "## Non-construct buckets (not in construct rankings)",
        "",
        "| Bucket | Fixtures (outside or partial) |",
        "| --- | ---: |",
    ]
    for bucket in sorted(NON_CONSTRUCT_BUCKETS):
        md.append(f"| `{bucket}` | {bucket_counts.get(bucket, 0)} |")

    md.extend(
        [
            "",
            "## Top first-refusal constructs (outside, constructs only)",
            "",
            "| Construct | Fixtures | Examples |",
            "| --- | ---: | --- |",
        ]
    )
    for construct, count in top_outside:
        if not construct:
            construct = "(empty)"
        examples = [
            r["fixture"]
            for r in rows
            if r.get("route_verdict") == "outside" and r.get("first_refusal", "") == construct
        ][:3]
        ex = ", ".join(f"`{e}`" for e in examples)
        md.append(f"| {construct} | {count} | {ex} |")

    md.extend(
        [
            "",
            "## Top first-refusal constructs (partial, constructs only)",
            "",
        ]
    )
    if top_partial:
        md.append("| Construct | Fixtures | Examples |")
        md.append("| --- | ---: | --- |")
        for construct, count in top_partial:
            if not construct:
                construct = "(empty)"
            examples = [
                r["fixture"]
                for r in rows
                if r.get("route_verdict") == "partial" and r.get("first_refusal", "") == construct
            ][:3]
            ex = ", ".join(f"`{e}`" for e in examples)
            md.append(f"| {construct} | {count} | {ex} |")
    else:
        md.append("_None._")

    pre = OUT_DIR / f"shard-{SHARD}.pre-pass-every.tsv"
    if pre.is_file():
        old: dict[str, str] = {}
        for line in pre.read_text().splitlines()[1:]:
            if not line.strip():
                continue
            fixture, *rest = line.split("\t")
            old[fixture] = rest[2] if len(rest) > 2 else ""
        changed_pass_run = 0
        for r in rows:
            if r["fixture"] not in old:
                continue
            if not (
                r["fixture"].startswith("tests/omega/pass/")
                or r["fixture"].startswith("tests/omega/run/")
            ):
                continue
            if old[r["fixture"]] != r.get("route_verdict", ""):
                changed_pass_run += 1
        md.extend(
            [
                "",
                "## pass/run `--every-machine` impact",
                "",
                f"Compared with the prior shard TSV (fail-only `--every-machine`): "
                f"**{changed_pass_run}** of **{sum(1 for r in rows if r['fixture'].startswith('tests/omega/pass/') or r['fixture'].startswith('tests/omega/run/'))}** "
                f"pass/run rows changed `route_verdict`.",
            ]
        )

    md.extend(
        [
            "",
            "## Route accepts / current rejects — soundness watch",
            "",
            "Harness `current=rejected` is the typed-frontier checker (`current-diagnostic` "
            "lines), not a later native stage. **Watch** = fixture `route_verdict=accepted` "
            "and `current_verdict=rejected`. Vacuous accepts (`obligations=0 judged=0` on "
            "every accepted root) are flagged separately: they are not holes in discharge "
            "but declaration/build rules the route does not judge.",
            "",
            "| Fixture | Tier | Diagnostic class | Vacuous? | Notes |",
            "| --- | --- | --- | --- | --- |",
        ]
    )
    for r in soundness:
        clazz, message = first_current_diagnostic(r["fixture"])
        label = vacuous_label(r)
        if label == "vacuous":
            note = (
                "vacuous — defect is outside the route's judged obligations; "
                f"diagnostic: {message[:200]}"
            )
        else:
            note = f"non-vacuous accept; diagnostic: {message[:200]}"
        md.append(
            f"| `{r['fixture']}` | {r.get('tier', '')} | {clazz} | {label} | {note} |"
        )
    if not soundness:
        md.append("| _none_ | | | | |")

    md.extend(["", "## All disagreements", ""])
    if disagreements:
        for r in disagreements[:500]:
            md.append(
                f"- `{r['fixture']}` route={r.get('route_verdict')} current={r.get('current_verdict')}"
            )
    else:
        md.append("_None._")

    md.extend(["", "## Timeouts", ""])
    if timeouts:
        for r in timeouts:
            md.append(f"- `{r['fixture']}` ({r.get('seconds', '?')}s)")
    else:
        md.append("_None.")

    md.extend(
        [
            "",
            "## Hand-check sample (5 disagreements)",
            "",
        ]
    )
    sample_disagree = [
        r for r in disagreements if not (r.get("route_verdict") == "accepted" and r.get("current_verdict") == "rejected")
    ][:5]
    for r in sample_disagree:
        md.append(f"### `{r['fixture']}`")
        md.append(
            f"- TSV: route={r.get('route_verdict')} current={r.get('current_verdict')} "
            f"agreement={r.get('agreement')}"
        )
        for note in hand_check_notes(r["fixture"]):
            md.append(f"- `{note}`")
        md.append("")

    if soundness:
        md.extend(["", "## Soundness watch harness traces", ""])
        for r in soundness:
            clazz, message = first_current_diagnostic(r["fixture"])
            md.append(f"### `{r['fixture']}`")
            md.append(f"- roots: `{r.get('roots', '')}`")
            md.append(f"- vacuous_accept_roots: `{r.get('vacuous_accept_roots', '')}`")
            md.append(f"- Diagnostic class: {clazz}")
            if message:
                md.append(f"- First message: {message[:300]}")
            for note in hand_check_notes(r["fixture"]):
                md.append(f"- `{note}`")
            md.append("")

    installed = [r["fixture"] for r in rows if "installed_arrival" in r["fixture"]]
    md.extend(["", "## installed_arrival_* fixtures in this shard", ""])
    if installed:
        for f in installed:
            match = next((r for r in rows if r["fixture"] == f), None)
            md.append(f"- `{f}` route_verdict={match.get('route_verdict') if match else '?'}")
    else:
        md.append("_None in shard 5._")

    MD.write_text("\n".join(md) + "\n")
    print(f"wrote {MD} ({total} rows)")


if __name__ == "__main__":
    main()
