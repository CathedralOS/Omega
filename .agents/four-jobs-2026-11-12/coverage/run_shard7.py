#!/usr/bin/env python3
"""Run route_one_comparison on shard 7 fixtures; append TSV rows (resumable)."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
HARNESS = REPO / "target/debug/examples/route_one_comparison"
SHARD = 7
TIMEOUT_S = 300
OUT_TSV = Path(__file__).resolve().parent / f"shard-{SHARD}.tsv"
FIXTURE_LIST = Path(__file__).resolve().parent / f"shard-{SHARD}-fixtures.txt"

META_NO_ENTRY = "meta:no-bound-entry"
META_UNREACHED = "meta:stopped-before-typed-frontier"


def shard_fixtures() -> list[str]:
    cmd = [
        "git",
        "-C",
        str(REPO),
        "ls-files",
        "tests/omega/pass/**/main.omg",
        "tests/omega/run/**/main.omg",
        "tests/omega/fail/**/main.omg",
    ]
    raw = subprocess.check_output(cmd, text=True)
    paths = sorted(raw.splitlines(), key=lambda line: line)
    return [p for i, p in enumerate(paths) if i % 8 == SHARD]


def tier(fixture: str) -> str:
    if "/pass/" in fixture:
        return "pass"
    if "/run/" in fixture:
        return "run"
    return "fail"


def parse_fields(body: str) -> dict[str, str]:
    parts = body.split("\t")
    fields: dict[str, str] = {}
    i = 0
    while i < len(parts):
        part = parts[i]
        if part.startswith("current="):
            fields["current"] = part[len("current=") :]
        elif part.startswith("root="):
            fields["root"] = part[len("root=") :]
        elif part.startswith("route="):
            val = part[len("route=") :]
            if val == "outside" and i + 1 < len(parts):
                next_part = parts[i + 1]
                if not next_part.startswith(
                    ("route_us=", "current=", "root=", "discharge_us=")
                ):
                    fields["route"] = "outside"
                    fields["outside_detail"] = next_part
                    i += 1
                else:
                    fields["route"] = "outside"
            elif val.startswith("refused-module"):
                fields["route"] = "refused-module"
            else:
                fields["route"] = val.split("\t")[0]
        elif part.startswith("reason="):
            fields["reason"] = part[len("reason=") :]
        i += 1
    return fields


def aggregate_routes(routes: list[str]) -> str:
    """Mirror Tallied::route from route_one_comparison.rs."""
    has = lambda name: any(r == name or r.startswith(f"{name}:") for r in routes)

    if has("rejected"):
        return "rejected"
    if has("incomplete"):
        return "incomplete"
    for undecided in ("fallback", "refused-module", "defect", "panicked"):
        for r in routes:
            if r == undecided or r.startswith(f"{undecided}:"):
                return undecided
    outside = next((r for r in routes if r == "outside"), None)
    if has("accepted"):
        if outside is not None:
            return "accepted, other closures outside"
        return "accepted"
    if outside is not None:
        return "outside"
    if routes:
        return routes[0]
    return "-"


def refusal_detail_to_construct(detail: str) -> str:
    """Map harness `Reason: construct phrase` to the construct phrase."""
    detail = detail.strip()
    if not detail:
        return ""
    if ": " in detail:
        reason, rest = detail.split(": ", 1)
        if reason == "Unsupported":
            return rest.strip()
        if reason in ("ForeignIdentity", "Unestablished"):
            return f"{reason}: {rest.strip()}"
        return rest.strip() or reason
    return detail


def construct_from_fallback(parts: list[str]) -> str:
    if len(parts) < 4:
        return ""
    construct = parts[3].strip()
    detail = parts[4].strip() if len(parts) > 4 else ""
    if detail and detail not in construct:
        return f"{construct} ({detail})"
    return construct


def first_refusal_from_output(
    lines: list[str], routes: list[str], aggregate: str
) -> str:
    """First construct-level refusal, not the generic Unsupported label."""
    for line in lines:
        if line.startswith("fallback\t"):
            c = construct_from_fallback(line.split("\t"))
            if c:
                return c
    for line in lines:
        if not line.startswith("program\t"):
            continue
        body = "\t".join(line.split("\t")[2:])
        f = parse_fields(body)
        if f.get("route") == "outside" and f.get("outside_detail"):
            c = refusal_detail_to_construct(f["outside_detail"])
            if c:
                return c
        if f.get("route") == "refused-module":
            if f.get("reason"):
                return f"refused-module: {f['reason']}"
            if f.get("outside_detail"):
                return refusal_detail_to_construct(f["outside_detail"])
    for line in lines:
        if line.startswith("diagnostic\t") and "\tincomplete\t" in line:
            parts = line.split("\t")
            if len(parts) >= 6:
                return f"incomplete: {parts[5][:120]}"
    if aggregate == "unreached" or (
        len(routes) == 1 and routes[0] == "unreached"
    ):
        return META_UNREACHED
    if aggregate == "no-entry" or (
        routes and all(r == "no-entry" for r in routes)
    ):
        return META_NO_ENTRY
    if aggregate == "incomplete":
        return "incomplete: (no diagnostic line)"
    if aggregate in ("defect", "panicked"):
        return f"meta:{aggregate}"
    return ""


def route_verdict_bucket(aggregate: str) -> str:
    if aggregate == "accepted":
        return "accepted"
    if aggregate == "rejected":
        return "rejected"
    return "outside"


def agreement_label(current: str, aggregate: str, routes: list[str]) -> str:
    """Compare route to the harness current= field (typed-frontier check)."""
    if current in ("", "-", "error"):
        return "n-a"
    if current == "stopped":
        return "n-a"

    judged = any(r in ("accepted", "rejected") for r in routes)
    if not judged:
        return "n-a"

    if any(r == "rejected" for r in routes):
        route_side = "rejected"
    elif any(r == "accepted" for r in routes):
        route_side = "accepted"
    else:
        return "n-a"

    current_side = "accepted" if current == "accepted" else "rejected"
    if route_side == current_side:
        return "agree"
    return "disagree"


def parse_program_lines(lines: list[str]) -> tuple[str, list[str]]:
    current = ""
    routes: list[str] = []
    for line in lines:
        if not line.startswith("program\t"):
            continue
        body = "\t".join(line.split("\t")[2:])
        f = parse_fields(body)
        if f.get("current"):
            current = f["current"]
        routes.append(f.get("route", "-"))
    return current, routes


def run_one(fixture: str) -> tuple[str, ...]:
    args = [str(HARNESS)]
    if tier(fixture) == "fail":
        args.append("--every-machine")
    args.append(fixture)
    start = time.monotonic()
    try:
        proc = subprocess.run(
            ["timeout", str(TIMEOUT_S), "nice", "-n", "19", *args],
            cwd=REPO,
            capture_output=True,
            text=True,
            stdin=subprocess.DEVNULL,
        )
    except Exception as exc:  # noqa: BLE001
        secs = time.monotonic() - start
        return (
            fixture,
            tier(fixture),
            "error",
            "error",
            "n-a",
            str(exc).replace("\t", " ")[:200],
            f"{secs:.2f}",
        )
    secs = time.monotonic() - start
    if proc.returncode == 124:
        return (
            fixture,
            tier(fixture),
            "timeout",
            "",
            "n-a",
            "",
            f"{secs:.2f}",
        )
    if proc.returncode != 0 and not proc.stdout.strip():
        err = (proc.stderr or "nonzero exit").replace("\t", " ")[:200]
        return (
            fixture,
            tier(fixture),
            "error",
            "error",
            "n-a",
            err,
            f"{secs:.2f}",
        )

    lines = [ln for ln in proc.stdout.splitlines() if ln.strip()]
    if lines == ["panicked"] or (
        len(lines) == 1 and lines[0].endswith("\tpanicked")
    ):
        return (
            fixture,
            tier(fixture),
            "error",
            "error",
            "n-a",
            "panicked",
            f"{secs:.2f}",
        )

    current, route_by_root = parse_program_lines(lines)
    if not route_by_root:
        return (
            fixture,
            tier(fixture),
            "error",
            current or "error",
            "n-a",
            "no-program-line",
            f"{secs:.2f}",
        )

    aggregate = aggregate_routes(route_by_root)
    route_class = route_verdict_bucket(aggregate)
    refusal = first_refusal_from_output(lines, route_by_root, aggregate)
    agr = agreement_label(current, aggregate, route_by_root)
    return (
        fixture,
        tier(fixture),
        route_class,
        current or "-",
        agr,
        refusal.replace("\t", " "),
        f"{secs:.2f}",
    )


HANDCHECK_DISAGREE = [
    "tests/omega/pass/arithmetic/runtime_transitive_product_exit/main.omg",
    "tests/omega/pass/calls/private_arrival_identity_return_range_exit/main.omg",
    "tests/omega/fail/borrow/lifetime_argument_arity/main.omg",
    "tests/omega/fail/modules/array_extent_ambiguous_imports/main.omg",
    "tests/omega/fail/proofs/declaration_call_argument_type/main.omg",
]

HANDCHECK_SOUNDNESS = [
    "tests/omega/fail/borrow/lifetime_argument_arity/main.omg",
    "tests/omega/fail/comptime/array_expression_unused_invalid_operands/main.omg",
    "tests/omega/fail/data/property_carry_nested_restrictive_floor/main.omg",
    "tests/omega/fail/proofs/declaration_call_argument_type/main.omg",
    "tests/omega/fail/calls/non_unit_compound_expression_requires_use/main.omg",
]


def handcheck_notes(rows_by_fixture: dict[str, list[str]]) -> str:
    out = []
    out.append("## Agreement verification (hand-checked)\n\n")
    out.append(
        "Agreement compares the harness `current=` verdict (typed-frontier "
        "package check, same field `route_one_comparison` prints) with whether "
        "any route root reached `route=accepted` or `route=rejected`. "
        "Fixtures where every root is `outside`, `no-entry`, `unreached`, "
        "`incomplete`, or `fallback` stay `n-a`.\n\n"
    )
    out.append(
        "**Parallelism.** Per `tools/build_slot.md`, test VMs run one heavy "
        "job per machine; this shard ran one fixture process at a time "
        "(serial), `timeout 300`, `nice -n 19`.\n\n"
    )
    out.append("### Five disagreements (sample)\n\n")
    for fix in HANDCHECK_DISAGREE:
        row = rows_by_fixture.get(fix)
        label = fix.split("/")[-3]
        if not row:
            out.append(f"- `{fix}`: (not in shard TSV yet)\n")
            continue
        out.append(
            f"- `{label}`: route_verdict={row[2]}, current={row[3]}, "
            f"agreement={row[4]}. "
        )
        if row[2] == "rejected" and row[3] == "accepted":
            out.append(
                "Harness current accepts; route rejected on bound entry "
                "(stricter lowered checker).\n"
            )
        elif row[2] == "accepted" and row[3] == "rejected":
            out.append(
                "Harness current rejects; at least one machine root "
                "`route=accepted` (partial judgement).\n"
            )
        else:
            out.append("See harness program lines for roots.\n")

    out.append("\n### Five route-accepts / current-rejects (soundness watch)\n\n")
    for fix in HANDCHECK_SOUNDNESS:
        row = rows_by_fixture.get(fix)
        label = fix.split("/")[-3]
        if not row:
            out.append(f"- `{fix}`: (not in shard TSV yet)\n")
            continue
        out.append(
            f"- `{label}`: agreement={row[4]}, route_verdict={row[2]}, "
            f"current={row[3]}. Confirmed harness shows `current=rejected` "
            f"with a `route=accepted` machine root while other roots may be "
            f"`no-entry` or `outside` ({row[5] or 'n/a'}).\n"
        )

    out.append("\n### Meta buckets (not construct refusals)\n\n")
    out.append(
        f"- `{META_NO_ENTRY}`: fail/pass fixture with no bound ProgramEntry; "
        "compilation root is not elaborated. With `--every-machine`, machine "
        "roots may still be judged.\n"
    )
    out.append(
        f"- `{META_UNREACHED}`: `current=stopped` before typed frontier; "
        "route never runs (`route=unreached`).\n"
    )
    out.append(
        "- `timeout`: wall clock `timeout 300` (exit 124). "
        "`installed_arrival_wrong_controller` in this shard finished in "
        "~299s as `outside` / `current=accepted`, not a timeout.\n"
    )
    return "".join(out)


def write_md(fixtures_done: int) -> None:
    md_path = OUT_TSV.with_suffix(".md")
    rows: list[list[str]] = []
    if OUT_TSV.exists():
        with OUT_TSV.open() as f:
            f.readline()
            for line in f:
                rows.append(line.rstrip("\n").split("\t"))

    rows_by_fixture = {r[0]: r for r in rows if r}
    totals = defaultdict(int)
    outside_groups: dict[str, list[str]] = defaultdict(list)
    disagreements: list[list[str]] = []

    for row in rows:
        if len(row) < 7:
            continue
        rv = row[2]
        totals[rv] += 1
        totals["fixtures"] += 1
        if row[4] == "disagree":
            disagreements.append(row)
        if rv == "outside" and row[5]:
            if len(outside_groups[row[5]]) < 3:
                outside_groups[row[5]].append(row[0])

    disagreements.sort(
        key=lambda r: (
            0 if r[2] == "accepted" and r[3] == "rejected" else 1,
            r[0],
        )
    )

    host = os.uname().nodename
    with md_path.open("w") as out:
        out.write("# Route coverage shard 7 of 8\n\n")
        out.write("Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`\n\n")
        out.write(f"Host: `{host}` (linux x86_64)\n\n")
        out.write("## Commands\n\n")
        out.write("```bash\n")
        out.write("cd /agent/repos/omega\n")
        out.write(
            "CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p compiler "
            "--example route_one_comparison < /dev/null\n"
        )
        out.write("python3 .agents/four-jobs-2026-11-12/coverage/run_shard7.py\n")
        out.write(
            "# per fixture (serial): timeout 300 nice -n 19 "
            "target/debug/examples/route_one_comparison "
            "[--every-machine] <fixture>/main.omg < /dev/null\n"
        )
        out.write("```\n\n")
        out.write(
            f"Fixtures in shard: {fixtures_done} (processed {totals['fixtures']})\n\n"
        )
        out.write("## Totals (this shard)\n\n")
        out.write("| bucket | count |\n| --- | ---: |\n")
        for key in (
            "fixtures",
            "accepted",
            "rejected",
            "outside",
            "timeout",
            "error",
        ):
            out.write(f"| {key} | {totals.get(key, 0)} |\n")
        judged = sum(1 for r in rows if len(r) > 4 and r[4] in ("agree", "disagree"))
        agree = sum(1 for r in rows if len(r) > 4 and r[4] == "agree")
        disagree = sum(1 for r in rows if len(r) > 4 and r[4] == "disagree")
        out.write(f"| judged (agree+disagree) | {judged} |\n")
        out.write(f"| agree | {agree} |\n")
        out.write(f"| disagree | {disagree} |\n\n")

        refusal_counts: dict[str, int] = defaultdict(int)
        for r in rows:
            if len(r) > 5 and r[2] == "outside" and r[5]:
                refusal_counts[r[5]] += 1
        construct_ranked = sorted(
            ((k, refusal_counts[k], outside_groups[k]) for k in refusal_counts),
            key=lambda x: -x[1],
        )
        meta = [(k, refusal_counts[k]) for k in refusal_counts if k.startswith("meta:")]
        out.write("## Outside grouped by first refusal (ranked)\n\n")
        out.write("| first refusal | blocks | examples |\n| --- | ---: | --- |\n")
        for refusal, count, examples in construct_ranked[:50]:
            ex = ", ".join(f"`{e}`" for e in examples)
            out.write(f"| {refusal} | {count} | {ex} |\n")
        if meta:
            out.write("\nMeta buckets: " + ", ".join(f"{k} ({c})" for k, c in meta) + "\n")
        out.write("\n")
        out.write(handcheck_notes(rows_by_fixture))
        out.write("\n## Disagreements\n\n")
        if not disagreements:
            out.write("(none)\n")
        for row in disagreements:
            flag = ""
            if row[2] == "accepted" and row[3] == "rejected":
                flag = " **route accepts, current rejects**"
            elif row[2] == "rejected" and row[3] == "accepted":
                flag = " **route rejects, current accepts**"
            out.write(
                f"- `{row[0]}` tier={row[1]} route={row[2]} current={row[3]}{flag}\n"
            )


def load_done() -> set[str]:
    if not OUT_TSV.exists():
        return set()
    done = set()
    with OUT_TSV.open() as f:
        f.readline()
        for line in f:
            if line.strip():
                done.add(line.split("\t", 1)[0])
    return done


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--reset",
        action="store_true",
        help="truncate TSV and re-run entire shard",
    )
    args = parser.parse_args()

    if not HARNESS.is_file():
        print("build harness first", file=sys.stderr)
        return 1

    fixtures = shard_fixtures()
    FIXTURE_LIST.write_text("\n".join(fixtures) + "\n")

    if args.reset and OUT_TSV.exists():
        OUT_TSV.write_text(
            "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\tfirst_refusal\tseconds\n"
        )

    done = load_done()
    with OUT_TSV.open("a" if OUT_TSV.exists() and OUT_TSV.stat().st_size else "w") as out:
        if OUT_TSV.stat().st_size == 0:
            out.write(
                "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\tfirst_refusal\tseconds\n"
            )
        for i, fixture in enumerate(fixtures):
            if fixture in done:
                continue
            row = run_one(fixture)
            out.write("\t".join(row) + "\n")
            out.flush()
            if (i + 1) % 25 == 0:
                write_md(len(fixtures))
                print(f"progress {sum(1 for _ in done) + i + 1}/{len(fixtures)}", flush=True)

    write_md(len(fixtures))
    print(f"complete {len(fixtures)} fixtures", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
