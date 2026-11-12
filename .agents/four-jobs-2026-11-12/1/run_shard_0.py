#!/usr/bin/env python3
"""Job 1 shard runner: route_one_comparison per fixture -> shard TSV."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import time
from collections import Counter
from concurrent.futures import FIRST_COMPLETED, ProcessPoolExecutor, wait
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


def parse_output(fixture: str, text: str, seconds: float, timed_out: bool, exit_code: int) -> str:
    if timed_out:
        return (
            f"{fixture}\t{tier(fixture)}\ttimeout\t-\tn/a\t-\t{TIMEOUT:.3f}\n"
        )
    if exit_code not in (0, 101) and not text.strip():
        return (
            f"{fixture}\t{tier(fixture)}\terror\t-\tn/a\t-\t{seconds:.3f}\n"
        )

    programs: list[dict[str, str]] = []
    fallbacks: list[str] = []
    first_outside_detail: str | None = None

    for line in text.splitlines():
        parts = line.split("\t")
        if not parts:
            continue
        kind = parts[0]
        if kind == "program" and len(parts) >= 3:
            route_detail: str | None = None
            for i, p in enumerate(parts):
                if p == "route=outside" and i + 1 < len(parts):
                    route_detail = parts[i + 1]
                    break
            fm = field_map(parts[2:])
            fm["_root"] = fm.get("root", "compilation")
            if route_detail is not None:
                fm["_outside_detail"] = route_detail
            programs.append(fm)
            if fm.get("route") == "outside" and first_outside_detail is None:
                first_outside_detail = route_detail
        elif kind == "fallback" and len(parts) >= 5:
            fallbacks.append(parts[4])

    if not programs:
        return (
            f"{fixture}\t{tier(fixture)}\terror\t-\tn/a\t-\t{seconds:.3f}\n"
        )

    routes = [p.get("route", "-") for p in programs]
    currents = {p.get("current", "-") for p in programs}
    current = next(iter(currents)) if len(currents) == 1 else programs[0].get("current", "-")

    judged_routes = {"accepted", "rejected", "incomplete", "fallback", "defect", "panicked"}

    if any(r == "rejected" for r in routes):
        route_verdict = "rejected"
    elif any(r == "accepted" for r in routes):
        route_verdict = "accepted"
    elif all(r in ("no-entry", "unreached", "outside", "refused-module") for r in routes):
        route_verdict = "outside"
    elif any(r in judged_routes for r in routes):
        route_verdict = "rejected" if "rejected" in routes else "outside"
    else:
        route_verdict = "outside"

    first_refusal = "-"
    has_machine = any(p.get("_root", "").startswith("machine:") for p in programs)
    for p in programs:
        r = p.get("route", "-")
        if r == "no-entry" and has_machine:
            continue
        if r in ("no-entry", "unreached"):
            first_refusal = r
            break
        if r == "outside":
            first_refusal = p.get("_outside_detail") or first_outside_detail or "outside"
            break
        if r == "refused-module":
            first_refusal = p.get("reason", "refused-module")
            break
        if r == "incomplete":
            first_refusal = "incomplete"
            break
        if r == "fallback":
            first_refusal = fallbacks[0] if fallbacks else "fallback"
            break
    if first_refusal == "-" and route_verdict == "outside":
        for r in routes:
            if r not in ("accepted", "rejected"):
                first_refusal = r
                break
    if route_verdict in ("accepted", "rejected"):
        first_refusal = "-"

    if current == "stopped":
        agreement = "n/a"
        current_verdict = "stopped"
    else:
        current_verdict = current
        cur_pos = current_verdict == "accepted"
        cur_neg = current_verdict == "rejected"
        route_pos = route_verdict == "accepted"
        route_neg = route_verdict == "rejected"
        if route_verdict == "outside":
            agreement = "n/a"
        elif (cur_pos and route_pos) or (cur_neg and route_neg):
            agreement = "agree"
        else:
            agreement = "disagree"

    return (
        f"{fixture}\t{tier(fixture)}\t{route_verdict}\t{current_verdict}\t"
        f"{agreement}\t{first_refusal}\t{seconds:.3f}\n"
    )


def run_one(fixture: str) -> tuple[str, str]:
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
            f"{fixture}\t{tier(fixture)}\terror\t-\tn/a\t{exc}\t{seconds:.3f}\n"
        )
        return fixture, row
    seconds = time.monotonic() - start
    timed_out = proc.returncode == 124
    text = proc.stdout or ""
    row = parse_output(fixture, text, seconds, timed_out, proc.returncode)
    raw_dir = REPO / ".agents/four-jobs-2026-11-12/1/raw"
    raw_dir.mkdir(parents=True, exist_ok=True)
    safe = fixture.replace("/", "__")
    (raw_dir / f"{safe}.tsv").write_text(text, encoding="utf-8")
    return fixture, row


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

    refusals = Counter(
        r[5] for r in rows if r[2] == "outside" and r[5] not in ("-", "")
    )
    top_refusals = refusals.most_common(25)

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
        "## Top first-refusal constructs (outside)",
        "",
        "| construct | count |",
        "| --- | ---: |",
    ]
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

    md_path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workers", type=int, default=2)
    parser.add_argument("--max", type=int, default=0, help="limit fixtures (debug)")
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

    done = load_done(tsv_path)
    pending = [f for f in fixtures if f not in done]

    if not tsv_path.exists():
        tsv_path.write_text(TSV_HEADER, encoding="utf-8")

    print(f"total_shard={len(fixtures)} done={len(done)} pending={len(pending)}", flush=True)

    if not pending:
        write_summary_md(tsv_path, md_path)
        return 0

    with ProcessPoolExecutor(max_workers=args.workers) as pool:
        futures = {pool.submit(run_one, f): f for f in pending}
        completed = 0
        while futures:
            done_set, _ = wait(futures, return_when=FIRST_COMPLETED)
            for fut in done_set:
                fixture = futures.pop(fut)
                try:
                    _, row = fut.result()
                except Exception as exc:  # noqa: BLE001
                    row = (
                        f"{fixture}\t{tier(fixture)}\terror\t-\tn/a\t{exc}\t0.000\n"
                    )
                with tsv_path.open("a", encoding="utf-8") as fh:
                    fh.write(row)
                completed += 1
                if completed % 25 == 0:
                    print(f"progress {completed}/{len(pending)}", flush=True)

    write_summary_md(tsv_path, md_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
