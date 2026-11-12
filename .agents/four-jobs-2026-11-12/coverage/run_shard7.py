#!/usr/bin/env python3
"""Run route_one_comparison on shard 7 fixtures; append TSV rows (resumable)."""

from __future__ import annotations

import os
import re
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

OUTSIDE_ROUTES = {
    "incomplete",
    "fallback",
    "no-entry",
    "unreached",
    "defect",
    "panicked",
    "refused-module",
}


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
                fields["route"] = "outside"
                fields["outside_detail"] = parts[i + 1]
                i += 1
            elif val.startswith("refused-module"):
                fields["route"] = "refused-module"
            else:
                fields["route"] = val.split("\t")[0]
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
                return r.split("\t")[0] if "\t" in r else r
    outside = next((r for r in routes if r == "outside" or r.startswith("outside")), None)
    if has("accepted"):
        if outside is not None:
            return "accepted, other closures outside"
        return "accepted"
    if outside is not None:
        return "outside"
    if routes:
        return routes[0]
    return "-"


def classify_route(aggregate: str) -> str:
    if aggregate == "accepted":
        return "accepted"
    if aggregate == "rejected":
        return "rejected"
    return "outside"


def first_refusal_from_output(lines: list[str], routes: list[str]) -> str:
    for line in lines:
        if line.startswith("fallback\t"):
            parts = line.split("\t")
            if len(parts) >= 4:
                return parts[3].strip()
    for line in lines:
        if line.startswith("program\t"):
            body = "\t".join(line.split("\t")[2:])
            f = parse_fields(body)
            if f.get("route") == "outside" and "outside_detail" in f:
                detail = f["outside_detail"]
                if ":" in detail:
                    return detail.split(":", 1)[0].strip()
                return detail.split()[0] if detail else ""
    for r in routes:
        if r.startswith("refused-module:"):
            return r.split(":", 1)[1]
        if r.startswith("outside:"):
            return r.split(":", 1)[1]
    if aggregate_routes(routes) in ("no-entry", "unreached"):
        return aggregate_routes(routes)
    return ""


def agreement(current: str, route_class: str) -> str:
    if route_class in ("outside", "timeout", "error"):
        return "n-a"
    if current == "stopped":
        return "n-a"
    cur = "accepted" if current == "accepted" else "rejected"
    if cur == route_class:
        return "agree"
    return "disagree"


def load_done() -> set[str]:
    if not OUT_TSV.exists():
        return set()
    done = set()
    with OUT_TSV.open() as f:
        header = f.readline()
        for line in f:
            if line.strip():
                done.add(line.split("\t", 1)[0])
    return done


def run_one(fixture: str) -> tuple[str, ...]:
    args = [str(HARNESS)]
    if tier(fixture) == "fail":
        args.append("--every-machine")
    args.append(fixture)
    start = time.monotonic()
    try:
        proc = subprocess.run(
            ["timeout", str(TIMEOUT_S), *args],
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

    route_by_root: list[str] = []
    current = ""
    for line in lines:
        if not line.startswith("program\t"):
            continue
        body = "\t".join(line.split("\t")[2:])
        f = parse_fields(body)
        if f.get("current"):
            current = f["current"]
        r = f.get("route", "-")
        if r == "refused-module" and "outside_detail" not in f:
            r = "refused-module"
        route_by_root.append(r)

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
    route_class = classify_route(aggregate)
    refusal = first_refusal_from_output(lines, route_by_root)
    agr = agreement(current, route_class)
    return (
        fixture,
        tier(fixture),
        route_class,
        current or "-",
        agr,
        refusal.replace("\t", " "),
        f"{secs:.2f}",
    )


def write_md(fixtures_done: int) -> None:
    """Write shard-7.md summary from TSV (partial OK during run)."""
    md_path = OUT_TSV.with_suffix(".md")
    rows: list[list[str]] = []
    if OUT_TSV.exists():
        with OUT_TSV.open() as f:
            header = f.readline()
            for line in f:
                rows.append(line.rstrip("\n").split("\t"))

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
        out.write(f"Measured commit: `0835b3aaf803ed97f905be42d94727ff58459263`\n\n")
        out.write(f"Host: `{host}` (linux x86_64)\n\n")
        out.write("## Commands\n\n")
        out.write("```bash\n")
        out.write("cd /agent/repos/omega\n")
        out.write(
            "CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p compiler --example route_one_comparison < /dev/null\n"
        )
        out.write(
            f"python3 .agents/four-jobs-2026-11-12/coverage/run_shard7.py\n"
        )
        out.write(
            "# per fixture: timeout 300 target/debug/examples/route_one_comparison "
            "[--every-machine] <fixture>/main.omg < /dev/null\n"
        )
        out.write("```\n\n")
        out.write(f"Fixtures in shard: {fixtures_done} (processed {totals['fixtures']})\n\n")
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
        judged = totals.get("accepted", 0) + totals.get("rejected", 0)
        agree = sum(1 for r in rows if len(r) > 4 and r[4] == "agree")
        disagree = sum(1 for r in rows if len(r) > 4 and r[4] == "disagree")
        out.write(f"| judged (accepted+rejected) | {judged} |\n")
        out.write(f"| agree | {agree} |\n")
        out.write(f"| disagree | {disagree} |\n\n")

        refusal_counts: dict[str, int] = defaultdict(int)
        for r in rows:
            if len(r) > 5 and r[2] == "outside" and r[5]:
                refusal_counts[r[5]] += 1
        ranked = sorted(
            (
                (k, refusal_counts[k], outside_groups[k])
                for k in refusal_counts
            ),
            key=lambda x: -x[1],
        )
        out.write("## Outside grouped by first refusal (ranked)\n\n")
        out.write("| first refusal | blocks | examples |\n| --- | ---: | --- |\n")
        for refusal, count, examples in ranked[:50]:
            ex = ", ".join(f"`{e}`" for e in examples)
            out.write(f"| {refusal} | {count} | {ex} |\n")
        out.write("\n## Disagreements\n\n")
        if not disagreements:
            out.write("(none yet)\n")
        for row in disagreements:
            flag = ""
            if row[2] == "accepted" and row[3] == "rejected":
                flag = " **route accepts, current rejects**"
            out.write(
                f"- `{row[0]}` tier={row[1]} route={row[2]} current={row[3]}{flag}\n"
            )


def main() -> int:
    if not HARNESS.is_file():
        print("build harness first", file=sys.stderr)
        return 1

    fixtures = shard_fixtures()
    FIXTURE_LIST.write_text("\n".join(fixtures) + "\n")
    done = load_done()

    write_header = not OUT_TSV.exists() or OUT_TSV.stat().st_size == 0
    mode = "a" if OUT_TSV.exists() else "w"
    with OUT_TSV.open(mode) as out:
        if write_header:
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
                print(f"progress {i + 1}/{len(fixtures)}", flush=True)

    write_md(len(fixtures))
    print(f"complete {len(fixtures)} fixtures", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
