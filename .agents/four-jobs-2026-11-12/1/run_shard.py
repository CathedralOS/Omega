#!/usr/bin/env python3
"""Run route_one_comparison on a fixture shard and append TSV rows."""
from __future__ import annotations

import os
import re
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
BIN = REPO / "target/debug/examples/route_one_comparison"
TIMEOUT_S = 300
SHARD = int(os.environ.get("SHARD", "6"))
PARTS = 8

DECIDED = ["rejected", "incomplete"]
UNDECIDED = ["fallback", "refused-module", "defect", "panicked"]


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
    paths = sorted(proc.stdout.splitlines(), key=lambda s: s)
    return [p for i, p in enumerate(paths) if i % PARTS == SHARD]


def field_prefix(line: str, prefix: str) -> str | None:
    for part in line.split("\t"):
        if part.startswith(prefix):
            return part[len(prefix) :]
    return None


def aggregate_route(routes: list[str]) -> str:
    def has(name: str) -> bool:
        return any(r.startswith(name) for r in routes)

    def first(name: str) -> str | None:
        for r in routes:
            if r.startswith(name):
                return r
        return None

    for name in DECIDED:
        if has(name):
            return name
    for name in UNDECIDED:
        if first(name):
            return first(name)  # type: ignore[return-value]
    if has("accepted") and first("outside"):
        return "accepted, other closures outside"
    if has("accepted"):
        return "accepted"
    out = first("outside")
    if out:
        return out
    if routes:
        return routes[0]
    return "-"


def normalize_route_verdict(raw: str) -> str:
    if raw == "timeout":
        return "timeout"
    if raw == "error":
        return "error"
    if raw.startswith("accepted"):
        return "accepted"
    if raw == "rejected":
        return "rejected"
    return "outside"


def current_to_verdict(current: str) -> str:
    if current in ("accepted", "rejected", "stopped"):
        return current
    return "error"


def agreement(current: str, route_v: str) -> str:
    if route_v in ("outside", "timeout", "error"):
        return "n-a"
    cur_ok = current == "accepted"
    cur_bad = current in ("rejected", "stopped")
    if route_v == "accepted" and cur_ok:
        return "agree"
    if route_v == "rejected" and cur_bad:
        return "agree"
    if route_v in ("accepted", "rejected"):
        return "disagree"
    return "n-a"


def first_refusal_from_output(lines: list[str], agg_route: str) -> str:
    if agg_route.startswith("outside:"):
        return agg_route.split(":", 1)[1]
    if agg_route.startswith("refused-module:"):
        return agg_route.split(":", 1)[1]
    for line in lines:
        if line.startswith("fallback\t"):
            parts = line.split("\t")
            if len(parts) >= 4:
                return parts[3].strip()
    machine_programs = [
        ln
        for ln in lines
        if ln.startswith("program\t") and "root=machine:" in ln
    ]
    scan = machine_programs if machine_programs else [ln for ln in lines if ln.startswith("program\t")]
    for line in scan:
        if not line.startswith("program\t"):
            continue
        route = field_prefix(line, "route=") or ""
        if route == "outside":
            parts = line.split("\t")
            idx = next((i for i, p in enumerate(parts) if p == "route=outside"), None)
            if idx is not None and idx + 1 < len(parts):
                detail = parts[idx + 1]
                return detail.split(":", 1)[0].strip()
        if route == "no-entry" and not machine_programs:
            return "no-entry"
        if route == "unreached":
            return "unreached"
        if route == "incomplete":
            return "incomplete"
        if route == "refused-module":
            reason = field_prefix(line, "reason=")
            if reason:
                return f"refused-module:{reason}"
    if agg_route in ("no-entry", "incomplete", "fallback", "defect", "panicked"):
        return agg_route
    if agg_route.startswith("accepted"):
        return ""
    if agg_route == "rejected":
        return ""
    if agg_route == "-":
        return "unknown"
    return ""


def tier(fixture: str) -> str:
    if "/pass/" in fixture:
        return "pass"
    if "/run/" in fixture:
        return "run"
    if "/fail/" in fixture:
        return "fail"
    return "unknown"


def run_one(fixture: str) -> tuple[str, str, str, str, str, str, float]:
    rel = fixture
    path = REPO / rel
    args = [str(BIN)]
    if tier(rel) == "fail":
        args.append("--every-machine")
    args.append(rel)
    start = time.monotonic()
    try:
        proc = subprocess.run(
            args,
            cwd=REPO,
            capture_output=True,
            text=True,
            timeout=TIMEOUT_S,
        )
    except subprocess.TimeoutExpired:
        return ("timeout", "", "n-a", "timeout", "timeout", time.monotonic() - start)
    seconds = time.monotonic() - start
    if proc.returncode != 0 and not proc.stdout.strip():
        return ("error", "", "n-a", "error", f"exit={proc.returncode}", seconds)
    out_lines = (proc.stdout + proc.stderr).splitlines()
    program_lines = [ln for ln in out_lines if ln.startswith("program\t")]
    if not program_lines:
        if any("panicked" in ln for ln in out_lines):
            return ("outside", "stopped", "n-a", "panicked", "panicked", seconds)
        return ("error", "", "n-a", "error", "no-program-line", seconds)

    routes: list[str] = []
    current = ""
    for pl in program_lines:
        c = field_prefix(pl, "current=")
        if c:
            current = c
        r = field_prefix(pl, "route=") or "-"
        if r == "outside":
            parts = pl.split("\t")
            idx = next((i for i, p in enumerate(parts) if p == "route=outside"), None)
            if idx is not None and idx + 1 < len(parts):
                detail = parts[idx + 1]
                reason = detail.split(":", 1)[0].strip()
                r = f"outside:{reason}"
        elif r == "refused-module":
            reason = field_prefix(pl, "reason=") or "-"
            r = f"refused-module:{reason}"
        routes.append(r)

    agg = aggregate_route(routes)
    raw_v = normalize_route_verdict(agg)
    cur_v = current_to_verdict(current)
    agree = agreement(cur_v, raw_v)
    refusal = first_refusal_from_output(out_lines, agg)
    return (raw_v, cur_v, agree, refusal, agg, seconds)


def summarize(tsv_path: Path, md_path: Path, pin: str, bin_path: Path) -> None:
    rows: list[dict[str, str]] = []
    for line in tsv_path.read_text().splitlines():
        if not line or line.startswith("fixture\t"):
            continue
        parts = line.split("\t")
        if len(parts) < 7:
            continue
        rows.append(
            {
                "fixture": parts[0],
                "tier": parts[1],
                "route_verdict": parts[2],
                "current_verdict": parts[3],
                "agreement": parts[4],
                "first_refusal": parts[5],
                "seconds": parts[6],
            }
        )
    from collections import Counter, defaultdict

    route_c = Counter(r["route_verdict"] for r in rows)
    agree_c = Counter(r["agreement"] for r in rows)
    refuse_blocks: dict[str, list[str]] = defaultdict(list)
    for r in rows:
        if r["route_verdict"] == "outside" and r["first_refusal"]:
            refuse_blocks[r["first_refusal"]].append(r["fixture"])

    disagreements = [r for r in rows if r["agreement"] == "disagree"]
    disagreements.sort(
        key=lambda r: (
            0 if r["current_verdict"] in ("rejected", "stopped") and r["route_verdict"] == "accepted" else 1,
            r["fixture"],
        )
    )

    host = subprocess.run(["hostname"], capture_output=True, text=True).stdout.strip()
    cmd_fail = f"timeout {TIMEOUT_S} {bin_path} --every-machine <fixture>"
    cmd_other = f"timeout {TIMEOUT_S} {bin_path} <fixture>"

    lines = [
        f"# Route coverage shard {SHARD} of {PARTS}",
        "",
        f"Measured commit: `{pin}`",
        f"Host: `{host}`",
        f"Harness: `{bin_path}`",
        "",
        "Build:",
        "```bash",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --example route_one_comparison < /dev/null",
        "```",
        "",
        "Run (per fixture, cwd repository root; `fail/` adds `--every-machine`):",
        f"- pass/run: `{cmd_other}`",
        f"- fail: `{cmd_fail}`",
        "",
        f"Shard selection: lines with 0-based index `i % {PARTS} == {SHARD}`.",
        f"Fixtures in shard: {len(rows)}",
        "",
        "## Totals",
        "",
        f"- judged (route accepted or rejected): {route_c.get('accepted', 0) + route_c.get('rejected', 0)}",
        f"- route accepted: {route_c.get('accepted', 0)}",
        f"- route rejected: {route_c.get('rejected', 0)}",
        f"- outside: {route_c.get('outside', 0)}",
        f"- timeout: {route_c.get('timeout', 0)}",
        f"- error: {route_c.get('error', 0)}",
        f"- agree: {agree_c.get('agree', 0)}",
        f"- disagree: {agree_c.get('disagree', 0)}",
        f"- agreement n/a: {agree_c.get('n-a', 0)}",
        "",
        "## First refusal ranks (outside)",
        "",
        "| first_refusal | blocks | examples |",
        "| --- | ---: | --- |",
    ]
    ranked = sorted(refuse_blocks.items(), key=lambda kv: (-len(kv[1]), kv[0]))
    for refusal, fixtures in ranked[:80]:
        ex = ", ".join(f"`{f}`" for f in fixtures[:3])
        lines.append(f"| `{refusal}` | {len(fixtures)} | {ex} |")
    if len(ranked) > 80:
        lines.append(f"| … | … | ({len(ranked) - 80} more refusal kinds omitted) |")

    lines.extend(["", "## Disagreements", ""])
    if not disagreements:
        lines.append("(none)")
    else:
        lines.append("| fixture | tier | current | route | first_refusal |")
        lines.append("| --- | --- | --- | --- | --- |")
        for r in disagreements:
            lines.append(
                f"| `{r['fixture']}` | {r['tier']} | {r['current_verdict']} | {r['route_verdict']} | `{r['first_refusal']}` |"
            )

  md_path.write_text("\n".join(lines) + "\n")


def main() -> None:
    out_dir = REPO / ".agents/four-jobs-2026-11-12/1"
    out_dir.mkdir(parents=True, exist_ok=True)
    tsv_path = out_dir / f"shard-{SHARD}.tsv"
    done: set[str] = set()
    if tsv_path.exists():
        for line in tsv_path.read_text().splitlines():
            if line and not line.startswith("fixture\t"):
                done.add(line.split("\t", 1)[0])

    fixtures = fixture_list()
    header = "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\tfirst_refusal\tseconds\n"
    if not tsv_path.exists():
        tsv_path.write_text(header)

    with tsv_path.open("a") as out:
        for i, fixture in enumerate(fixtures):
            if fixture in done:
                continue
            rv, cv, agree, refusal, _agg, secs = run_one(fixture)
            row = (
                f"{fixture}\t{tier(fixture)}\t{rv}\t{cv}\t{agree}\t{refusal}\t{secs:.3f}\n"
            )
            out.write(row)
            out.flush()
            if (i + 1) % 25 == 0:
                print(f"[shard {SHARD}] {i + 1}/{len(fixtures)}", flush=True)

    pin = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=REPO, capture_output=True, text=True, check=True
    ).stdout.strip()
    summarize(tsv_path, out_dir / f"shard-{SHARD}.md", pin, BIN)
    print(f"done shard {SHARD}: {len(fixtures)} fixtures", flush=True)


if __name__ == "__main__":
    main()
