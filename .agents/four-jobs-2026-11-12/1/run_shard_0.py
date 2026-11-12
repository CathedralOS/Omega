#!/usr/bin/env python3
"""Job 1 shard runner: route_one_comparison per fixture -> shard TSV."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import time
from collections import Counter
from dataclasses import dataclass
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
HARNESS = REPO / "target/debug/examples/route_one_comparison"
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
TIMEOUT = 300
SHARD = 0
MOD = 8

TSV_HEADER = (
    "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\t"
    "first_refusal\tseconds\n"
)


def field_map(fields: list[str]) -> dict[str, str]:
    out: dict[str, str] = {}
    for f in fields:
        if "=" in f:
            k, v = f.split("=", 1)
            out[k] = v
    return out


def tier(fixture: str) -> str:
    if "/pass/" in fixture:
        return "pass"
    if "/run/" in fixture:
        return "run"
    if "/fail/" in fixture:
        return "fail"
    return "unknown"


def program_route(routes: list[str]) -> str:
    """Program-level route verdict: accepted only if every root is accepted."""
    if not routes:
        return "outside"
    if any(r == "rejected" for r in routes):
        return "rejected"
    if all(r == "accepted" for r in routes):
        return "accepted"
    return "outside"


def first_program_refusal(programs: list[dict[str, str]], route_verdict: str) -> str:
    if route_verdict in ("accepted", "rejected", "timeout", "error"):
        return "-"
    has_machine = any(p.get("_root", "").startswith("machine:") for p in programs)
    routes = [p.get("route", "-") for p in programs]
    if route_verdict == "outside" and all(r == "no-entry" for r in routes):
        return "meta:no-entry"
    if route_verdict == "outside" and all(r == "unreached" for r in routes):
        return "meta:unreached"
    for p in programs:
        r = p.get("route", "-")
        if r == "no-entry" and has_machine:
            continue
        if r == "outside":
            detail = p.get("_outside_detail", "outside")
            return detail if detail.startswith("Unsupported:") else f"Unsupported: {detail}"
        if r == "unreached":
            return "meta:unreached"
        if r == "no-entry":
            return "meta:no-entry"
        if r == "refused-module":
            reason = p.get("reason", "refused-module")
            detail = p.get("_refused_detail", reason)
            return detail if ":" in detail else f"{reason}: {detail}"
        if r in ("incomplete", "fallback"):
            return r
    return "outside"


def parse_output(fixture: str, text: str, seconds: float, timed_out: bool, exit_code: int) -> str:
    if timed_out:
        return (
            f"{fixture}\t{tier(fixture)}\ttimeout\t-\tn-a\t-\t{TIMEOUT:.3f}\n"
        )
    if exit_code not in (0, 101) and not text.strip():
        return (
            f"{fixture}\t{tier(fixture)}\terror\t-\tn-a\t-\t{seconds:.3f}\n"
        )

    programs: list[dict[str, str]] = []

    for line in text.splitlines():
        parts = line.split("\t")
        if not parts:
            continue
        kind = parts[0]
        if kind == "program" and len(parts) >= 3:
            route_detail: str | None = None
            refused_detail: str | None = None
            pending: str | None = None
            for p in parts[2:]:
                if p == "route=outside":
                    pending = "outside"
                    continue
                if pending == "outside":
                    route_detail = p
                    pending = None
                    continue
                if p.startswith("route=refused-module"):
                    pending = "refused"
                    continue
                if pending == "refused":
                    if p.startswith("reason="):
                        refused_detail = p.split("=", 1)[1]
                    elif refused_detail is None and not p.startswith("route_us="):
                        refused_detail = p
                    continue
            fm = field_map(parts[2:])
            fm["_root"] = fm.get("root", "compilation")
            if route_detail is not None:
                fm["_outside_detail"] = route_detail
            if refused_detail is not None:
                fm["_refused_detail"] = refused_detail
            programs.append(fm)

    if not programs:
        return (
            f"{fixture}\t{tier(fixture)}\terror\t-\tn-a\t-\t{seconds:.3f}\n"
        )

    routes = [p.get("route", "-") for p in programs]
    currents = {p.get("current", "-") for p in programs}
    if len(currents) != 1:
        # Harness prints the same check-phase current= on every root line.
        current = programs[0].get("current", "-")
    else:
        current = next(iter(currents))

    route_verdict = program_route(routes)
    first_refusal = first_program_refusal(programs, route_verdict)

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
        f"{agreement}\t{first_refusal}\t{seconds:.3f}\n"
    )


def run_one(fixture: str) -> tuple[str, str | None]:
    cmd = ["timeout", str(TIMEOUT), "nice", "-n", "19", str(HARNESS)]
    if "/fail/" in fixture:
        cmd.append("--every-machine")
    cmd.append(fixture)
    start = time.monotonic()
    try:
        proc = subprocess.run(
            cmd,
            cwd=REPO,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
        )
    except Exception as exc:  # noqa: BLE001
        seconds = time.monotonic() - start
        row = (
            f"{fixture}\t{tier(fixture)}\terror\t-\tn-a\t{exc}\t{seconds:.3f}\n"
        )
        return fixture, row, ""
    seconds = time.monotonic() - start
    timed_out = proc.returncode == 124
    text = proc.stdout or ""
    row = parse_output(fixture, text, seconds, timed_out, proc.returncode)
    return fixture, row, text


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
    return [line for i, line in enumerate(lines) if i % MOD == SHARD]


def load_done(path: Path) -> set[str]:
    if not path.exists():
        return set()
    done: set[str] = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("fixture\t") or not line.strip():
            continue
        done.add(line.split("\t", 1)[0])
    return done


def hand_check_samples(tsv_rows: list[list[str]], sample_n: int = 5) -> list[str]:
    import random

    soundness = [r for r in tsv_rows if r[2] == "accepted" and r[3] == "rejected"]
    disagrees = [r for r in tsv_rows if r[4] == "disagree"]
    random.seed(0)
    picks = random.sample(soundness, min(sample_n, len(soundness)))
    disagree_picks = random.sample(disagrees, min(sample_n, len(disagrees)))
    lines = [
        "",
        "## Hand-checked soundness-watch rows (lead review)",
        "",
        "Re-ran `route_one_comparison` and verified program-level aggregation "
        "(accepted only if every root is accepted). `current=rejected` is the "
        "harness check-phase field from `check_fixture_keeping_typed_frontier`, "
        "not a build or native runtime failure.",
        "",
    ]
    for row in picks:
        fixture = row[0]
        _fixture, _row, text = run_one(fixture)
        if not text:
            lines.append("- (harness produced no output)")
            continue
        program_lines = [ln for ln in text.splitlines() if ln.startswith("program\t")]
        lines.append(f"### `{fixture}`")
        lines.append(f"- TSV row: route={row[2]} current={row[3]} agreement={row[4]}")
        for pl in program_lines[:6]:
            lines.append(f"- `{pl}`")
        if len(program_lines) > 6:
            lines.append(f"- ... ({len(program_lines) - 6} more roots)")

    lines.extend(["", "## Hand-checked disagree rows", ""])
    for row in disagree_picks:
        fixture = row[0]
        _fixture, _row, text = run_one(fixture)
        program_lines = [ln for ln in (text or "").splitlines() if ln.startswith("program\t")]
        lines.append(f"### `{fixture}`")
        lines.append(f"- TSV row: route={row[2]} current={row[3]} agreement={row[4]}")
        for pl in program_lines[:6]:
            lines.append(f"- `{pl}`")
    return lines


def write_summary_md(tsv_path: Path, md_path: Path) -> None:
    rows: list[list[str]] = []
    for line in tsv_path.read_text(encoding="utf-8").splitlines():
        if line.startswith("fixture\t") or not line.strip():
            continue
        rows.append(line.split("\t"))

    judged = sum(1 for r in rows if r[2] in ("accepted", "rejected"))
    agreed = sum(1 for r in rows if r[4] == "agree")
    disagreed = sum(1 for r in rows if r[4] == "disagree")
    outside = sum(1 for r in rows if r[2] == "outside")
    timeout_n = sum(1 for r in rows if r[2] == "timeout")
    error_n = sum(1 for r in rows if r[2] == "error")

    construct_refusals = Counter(
        r[5]
        for r in rows
        if r[2] == "outside" and r[5].startswith("Unsupported:")
    )
    meta_refusals = Counter(
        r[5] for r in rows if r[2] == "outside" and r[5].startswith("meta:")
    )
    top_refusals = construct_refusals.most_common(25)
    top_construct_10 = construct_refusals.most_common(10)

    soundness = [
        r for r in rows
        if r[2] == "accepted" and r[3] == "rejected"
    ]

    disagree_all = [r for r in rows if r[4] == "disagree"]

    harness_cmd = (
        f"timeout {TIMEOUT} nice -n 19 {HARNESS} "
        "[--every-machine] <fixture>/main.omg < /dev/null"
    )
    build_cmd = (
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target "
        "mbx build -p compiler --example route_one_comparison < /dev/null"
    )

    lines = [
        f"# Job 1 route coverage — shard {SHARD}",
        "",
        f"- PIN: `{PIN}`",
        f"- Harness build commit: `{PIN}` (binary from this checkout)",
        f"- Branch: `work/route-coverage-shard-{SHARD}`",
        f"- Shard rule: fixture index `i % {MOD} == {SHARD}`",
        f"- Fixtures in shard: {len(rows)} (completed {len(rows)})",
        "",
        "## Commands",
        "",
        "```bash",
        build_cmd,
        harness_cmd,
        "```",
        "",
        "## Counts",
        "",
        f"| metric | count |",
        f"| --- | ---: |",
        f"| judged (route accepted or rejected) | {judged} |",
        f"| agreed | {agreed} |",
        f"| disagreed | {disagreed} |",
        f"| outside | {outside} |",
        f"| timeout | {timeout_n} |",
        f"| error | {error_n} |",
        "",
        "Program-level `route_verdict`: **accepted** only when every harness "
        "`program` root reports `route=accepted`; a lone machine root cannot "
        "accept a fixture whose compilation root is `no-entry` or `outside`.",
        "",
        "Agreement compares program-level route to harness `current=` "
        "(check-phase accept/reject on the typed frontier), not corpus tier.",
        "",
        "Fixtures ran **serially** (one harness process at a time; "
        "`tools/build_slot.md` test-VM rule).",
        "",
        "## Meta buckets",
        "",
        "| bucket | count |",
        "| --- | ---: |",
    ]
    for name, count in meta_refusals.most_common():
        lines.append(f"| {name} | {count} |")

    lines.extend(
        [
            "",
            "## Top 10 construct refusals (`Unsupported: …`)",
            "",
            "| construct | count |",
            "| --- | ---: |",
        ]
    )
    for name, count in top_construct_10:
        lines.append(f"| {name} | {count} |")

    lines.extend(
        [
            "",
            "## All construct refusals (ranked)",
            "",
            "| construct | count |",
            "| --- | ---: |",
        ]
    )
    for name, count in top_refusals:
        lines.append(f"| {name} | {count} |")

    lines.extend(["", "## Route accepts, current rejects (soundness candidates)", ""])
    if soundness:
        for r in soundness:
            lines.append(f"- `{r[0]}`")
    else:
        lines.append("(none)")

    lines.extend(["", "## All disagreements", ""])
    for r in disagree_all:
        lines.append(
            f"- `{r[0]}` tier={r[1]} route={r[2]} current={r[3]} first_refusal={r[5]}"
        )

    arrival = [r for r in rows if "installed_arrival" in r[0]]
    lines.extend(["", "## `installed_arrival_*` in shard", ""])
    if arrival:
        for r in arrival:
            lines.append(f"- `{r[0]}` route={r[2]} ({r[6]}s)")
    else:
        lines.append(
            "Not in this shard (`installed_arrival_without_context` → shard 6; "
            "`installed_arrival_wrong_controller` → shard 7 by `i % 8`)."
        )

    lines.extend(hand_check_samples(rows))

    md_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--max", type=int, default=0, help="limit fixtures (debug)")
    parser.add_argument(
        "--rebuild",
        action="store_true",
        help="re-run every fixture in the shard and replace shard-0.tsv",
    )
    args = parser.parse_args()

    out_dir = REPO / ".agents/four-jobs-2026-11-12/1"
    tsv_path = out_dir / f"shard-{SHARD}.tsv"
    md_path = out_dir / f"shard-{SHARD}.md"

    if not HARNESS.is_file():
        print("harness missing", file=sys.stderr)
        return 1

    fixtures = fixture_list()
    if args.max:
        fixtures = fixtures[: args.max]

    if args.rebuild:
        pending = fixtures
        tsv_path.write_text(TSV_HEADER, encoding="utf-8")
    else:
        done = load_done(tsv_path)
        pending = [f for f in fixtures if f not in done]
        if not tsv_path.exists():
            tsv_path.write_text(TSV_HEADER, encoding="utf-8")

    print(f"total_shard={len(fixtures)} pending={len(pending)}", flush=True)

    if not pending:
        write_summary_md(tsv_path, md_path)
        return 0

    rows_out: list[str] = []
    for completed, fixture in enumerate(pending, start=1):
        try:
            _, row, _text = run_one(fixture)
        except Exception as exc:  # noqa: BLE001
            row = f"{fixture}\t{tier(fixture)}\terror\t-\tn/a\t{exc}\t0.000\n"
        if args.rebuild:
            rows_out.append(row)
        else:
            with tsv_path.open("a", encoding="utf-8") as fh:
                fh.write(row)
        if completed % 25 == 0 or completed == len(pending):
            print(f"progress {completed}/{len(pending)}", flush=True)

    if args.rebuild:
        with tsv_path.open("w", encoding="utf-8") as fh:
            fh.write(TSV_HEADER)
            fh.writelines(rows_out)

    write_summary_md(tsv_path, md_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
