#!/usr/bin/env python3
"""Job 1 shard 0: route_one_comparison → TSV + summary (serial harness)."""

from __future__ import annotations

import argparse
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
OUT_DIR = REPO / ".agents/four-jobs-2026-11-12/route-coverage"
RAW_DIR = OUT_DIR / "raw"
HARNESS = REPO / "target/debug/examples/route_one_comparison"
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
TIMEOUT = 300
SHARD = 0
MOD = 8

UNJUDGED = frozenset(
    {
        "outside",
        "no-entry",
        "unreached",
        "incomplete",
        "fallback",
        "refused-module",
        "defect",
        "panicked",
        "error",
    }
)

TSV_HEADER = (
    "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\t"
    "first_refusal\troot_verdicts\tvacuous_all_accepted\tseconds\n"
)


def field_map(fields: list[str]) -> dict[str, str]:
    out: dict[str, str] = {}
    for field in fields:
        if "=" in field:
            key, value = field.split("=", 1)
            out[key] = value
    return out


def tier(fixture: str) -> str:
    if "/pass/" in fixture:
        return "pass"
    if "/run/" in fixture:
        return "run"
    if "/fail/" in fixture:
        return "fail"
    return "unknown"


def parse_program_lines(fixture: str, text: str) -> list[dict[str, str]]:
    programs: list[dict[str, str]] = []
    for line in text.splitlines():
        parts = line.split("\t")
        if not parts or parts[0] != "program" or len(parts) < 3 or parts[1] != fixture:
            continue
        route_detail: str | None = None
        refused_detail: str | None = None
        pending: str | None = None
        for part in parts[2:]:
            if part == "route=outside":
                pending = "outside"
                continue
            if pending == "outside":
                route_detail = part
                pending = None
                continue
            if part.startswith("route=refused-module"):
                pending = "refused"
                continue
            if pending == "refused":
                if part.startswith("reason="):
                    refused_detail = part.split("=", 1)[1]
                elif refused_detail is None and not part.startswith("route_us="):
                    refused_detail = part
                continue
        row = field_map(parts[2:])
        row["_root"] = row.get("root", "compilation")
        if route_detail is not None:
            row["_outside_detail"] = route_detail
        if refused_detail is not None:
            row["_refused_detail"] = refused_detail
        programs.append(row)
    return programs


def program_route(routes: list[str]) -> str:
    if not routes:
        return "outside"
    if any(r == "rejected" for r in routes):
        return "rejected"
    if all(r == "accepted" for r in routes):
        return "accepted"
    if any(r == "accepted" for r in routes) and any(
        r in UNJUDGED or r not in ("accepted", "rejected") for r in routes
    ):
        return "partial"
    return "outside"


def root_verdicts_str(programs: list[dict[str, str]]) -> str:
    pairs: list[str] = []
    for program in programs:
        root = program.get("_root", "compilation")
        route = program.get("route", "-")
        pairs.append(f"{root}:{route}")
    return ";".join(pairs)


def vacuous_all_accepted(programs: list[dict[str, str]]) -> str:
    accepted = [p for p in programs if p.get("route") == "accepted"]
    if not accepted:
        return "no"

    def vacuous(program: dict[str, str]) -> bool:
        return program.get("obligations") == "0" and program.get("judged") == "0"

    return "yes" if all(vacuous(p) for p in accepted) else "no"


def first_program_refusal(
    programs: list[dict[str, str]], route_verdict: str
) -> str:
    if route_verdict in ("accepted", "rejected", "timeout", "error"):
        return "-"
    routes = [p.get("route", "-") for p in programs]
    has_machine = any(p.get("_root", "").startswith("machine:") for p in programs)

    if route_verdict == "outside":
        if all(r == "no-entry" for r in routes):
            return "meta:no-entry"
        if all(r == "unreached" for r in routes):
            return "meta:unreached"

    for program in programs:
        route = program.get("route", "-")
        if route == "no-entry" and has_machine:
            continue
        if route == "outside":
            detail = program.get("_outside_detail", "outside")
            if detail.startswith("Unsupported:"):
                return detail
            return f"Unsupported: {detail}"
        if route == "unreached":
            return "meta:unreached"
        if route == "no-entry":
            return "meta:no-entry"
        if route == "refused-module":
            reason = program.get("reason", "refused-module")
            detail = program.get("_refused_detail", reason)
            return detail if ":" in detail else f"{reason}: {detail}"
        if route in ("incomplete", "fallback"):
            return route
    return "outside"


def parse_output(
    fixture: str,
    text: str,
    seconds: float,
    timed_out: bool,
    exit_code: int,
) -> str:
    if timed_out:
        return (
            f"{fixture}\t{tier(fixture)}\ttimeout\t-\tn-a\t-\t-\tno\t"
            f"{TIMEOUT:.3f}\n"
        )
    if exit_code not in (0, 101) and not text.strip():
        return (
            f"{fixture}\t{tier(fixture)}\terror\t-\tn-a\t-\t-\tno\t"
            f"{seconds:.3f}\n"
        )

    programs = parse_program_lines(fixture, text)
    if not programs:
        return (
            f"{fixture}\t{tier(fixture)}\terror\t-\tn-a\t-\t-\tno\t"
            f"{seconds:.3f}\n"
        )

    routes = [p.get("route", "-") for p in programs]
    currents = {p.get("current", "-") for p in programs}
    current = next(iter(currents)) if len(currents) == 1 else programs[0].get("current", "-")

    route_verdict = program_route(routes)
    first_refusal = first_program_refusal(programs, route_verdict)
    roots = root_verdicts_str(programs)
    vacuous = vacuous_all_accepted(programs)

    if current == "stopped":
        agreement = "n-a"
        current_verdict = "stopped"
    else:
        current_verdict = current
        if route_verdict not in ("accepted", "rejected"):
            agreement = "n-a"
        elif route_verdict == current_verdict:
            agreement = "agree"
        else:
            agreement = "disagree"

    return (
        f"{fixture}\t{tier(fixture)}\t{route_verdict}\t{current_verdict}\t"
        f"{agreement}\t{first_refusal}\t{roots}\t{vacuous}\t{seconds:.3f}\n"
    )


def run_harness(fixture: str) -> tuple[str, float, bool, int]:
    cmd = ["timeout", str(TIMEOUT), "nice", "-n", "19", str(HARNESS)]
    if "/fail/" in fixture:
        cmd.append("--every-machine")
    cmd.append(fixture)
    start = time.monotonic()
    proc = subprocess.run(
        cmd,
        cwd=REPO,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
    )
    seconds = time.monotonic() - start
    return proc.stdout or "", seconds, proc.returncode == 124, proc.returncode


def fixture_list() -> list[str]:
    proc = subprocess.run(
        [
            "git",
            "ls-files",
            "tests/omega/pass/**/main.omg",
            "tests/omega/run/**/main.omg",
            "tests/omega/fail/**/main.omg",
        ],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=True,
    )
    lines = sorted(proc.stdout.splitlines(), key=lambda s: s)
    return [line for index, line in enumerate(lines) if index % MOD == SHARD]


def raw_path(fixture: str) -> Path:
    return RAW_DIR / fixture.replace("/", "__")


def fixture_from_raw(path: Path) -> str:
    return path.name.replace("__", "/")


def write_summary_md(tsv_path: Path, md_path: Path) -> None:
    rows: list[list[str]] = []
    for line in tsv_path.read_text(encoding="utf-8").splitlines():
        if line.startswith("fixture\t") or not line.strip():
            continue
        rows.append(line.split("\t"))

    accepted_n = sum(1 for r in rows if r[2] == "accepted")
    rejected_n = sum(1 for r in rows if r[2] == "rejected")
    judged = accepted_n + rejected_n
    partial_n = sum(1 for r in rows if r[2] == "partial")
    outside_n = sum(1 for r in rows if r[2] == "outside")
    timeout_n = sum(1 for r in rows if r[2] == "timeout")
    error_n = sum(1 for r in rows if r[2] == "error")
    agreed = sum(1 for r in rows if r[4] == "agree")
    disagreed = sum(1 for r in rows if r[4] == "disagree")
    vacuous_yes = sum(1 for r in rows if len(r) > 8 and r[7] == "yes")

    construct_refusals = Counter(
        r[5]
        for r in rows
        if r[2] == "outside" and r[5].startswith("Unsupported:")
    )
    partial_refusals = Counter(
        r[5]
        for r in rows
        if r[2] == "partial" and r[5] not in ("-", "") and not r[5].startswith("meta:")
    )
    meta_refusals = Counter(
        r[5] for r in rows if r[2] == "outside" and r[5].startswith("meta:")
    )

    soundness = [r for r in rows if r[2] == "accepted" and r[3] == "rejected"]
    disagree_all = [r for r in rows if r[4] == "disagree"]
    total_check = judged + partial_n + outside_n + timeout_n + error_n

    lines = [
        f"# Job 1 route coverage — shard {SHARD}",
        "",
        f"- PIN: `{PIN}`",
        f"- Branch: `work/route-coverage-shard-{SHARD}`",
        f"- Shard rule: fixture index `i % {MOD} == {SHARD}`",
        f"- Fixtures: {len(rows)} (checksum {total_check})",
        "",
        "## Program verdict rules",
        "",
        "- Any root `rejected` → program **rejected** (judged).",
        "- All roots `accepted` → program **accepted** (judged).",
        "- `accepted` mixed with outside / no-entry / unreached / unjudged, no "
        "`rejected` → **partial** (not in agreement denominator).",
        "- Otherwise **outside** (or timeout/error).",
        "",
        "## Counts",
        "",
        "| metric | count |",
        "| --- | ---: |",
        f"| accepted (judged) | {accepted_n} |",
        f"| rejected (judged) | {rejected_n} |",
        f"| judged (accepted + rejected) | {judged} |",
        f"| agree | {agreed} |",
        f"| disagree | {disagreed} |",
        f"| partial | {partial_n} |",
        f"| outside | {outside_n} |",
        f"| timeout | {timeout_n} |",
        f"| error | {error_n} |",
        f"| vacuous (every accepted root obligations=0 judged=0) | {vacuous_yes} |",
        "",
        f"Identity: agree + disagree = judged ({agreed + disagreed} = {judged}).",
        f"Identity: outside + partial + judged + timeout + error = {total_check}.",
        "",
        "## Non-construct buckets (outside only)",
        "",
        "| bucket | count |",
        "| --- | ---: |",
    ]
    for name, count in meta_refusals.most_common():
        lines.append(f"| {name} | {count} |")

    lines.extend(
        [
            "",
            "## Top 10 construct refusals (outside rows only)",
            "",
            "| construct | count |",
            "| --- | ---: |",
        ]
    )
    for name, count in construct_refusals.most_common(10):
        lines.append(f"| {name} | {count} |")

    lines.extend(
        [
            "",
            "## Top partial first-refusal constructs",
            "",
            "| first refusal | count |",
            "| --- | ---: |",
        ]
    )
    for name, count in partial_refusals.most_common(15):
        lines.append(f"| {name} | {count} |")

    lines.extend(["", "## Route accepts, current rejects (soundness candidates)", ""])
    lines.append("(none)" if not soundness else "\n".join(f"- `{r[0]}`" for r in soundness))

    lines.extend(["", "## All disagreements", ""])
    for row in disagree_all:
        lines.append(
            f"- `{row[0]}` tier={row[1]} route={row[2]} current={row[3]} "
            f"roots={row[6] if len(row) > 6 else '-'}"
        )

    lines.extend(
        [
            "",
            "## Commands",
            "",
            "```bash",
            "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target "
            "mbx build -p compiler --example route_one_comparison < /dev/null",
            f"python3 .agents/four-jobs-2026-11-12/route-coverage/reparse_shard_0.py",
            "```",
        ]
    )
    md_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--max", type=int, default=0)
    args = parser.parse_args()

    tsv_path = OUT_DIR / f"shard-{SHARD}.tsv"
    md_path = OUT_DIR / f"shard-{SHARD}.md"
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RAW_DIR.mkdir(parents=True, exist_ok=True)

    if not HARNESS.is_file():
        print("harness missing", file=sys.stderr)
        return 1

    fixtures = fixture_list()[: args.max] if args.max else fixture_list()
    rows_out: list[str] = []
    for index, fixture in enumerate(fixtures, start=1):
        text, seconds, timed_out, exit_code = run_harness(fixture)
        raw_path(fixture).write_text(text, encoding="utf-8")
        rows_out.append(parse_output(fixture, text, seconds, timed_out, exit_code))
        if index % 25 == 0 or index == len(fixtures):
            print(f"progress {index}/{len(fixtures)}", flush=True)

    tsv_path.write_text(TSV_HEADER + "".join(rows_out), encoding="utf-8")
    write_summary_md(tsv_path, md_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
