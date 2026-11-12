#!/usr/bin/env python3
"""Run route_one_comparison on shard fixtures; append TSV rows with resume."""

from __future__ import annotations

import re
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
HARNESS = REPO / "target/debug/examples/route_one_comparison"
OUT_DIR = Path(__file__).resolve().parent
SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 1
TSV = OUT_DIR / f"shard-{SHARD}.tsv"
TIMEOUT_S = 300

HEADER = (
    "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\tfirst_refusal\tseconds\n"
)


def tier_of(fixture: str) -> str:
    if "/pass/" in fixture:
        return "pass"
    if "/run/" in fixture:
        return "run"
    return "fail"


def shard_fixtures(shard: int) -> list[str]:
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
    return [line for i, line in enumerate(lines) if i % 8 == shard]


def field_map(parts: list[str]) -> dict[str, str]:
    out: dict[str, str] = {}
    i = 0
    while i < len(parts):
        part = parts[i]
        if part == "route=outside":
            out["route"] = "outside"
            if i + 1 < len(parts):
                out["outside_detail"] = parts[i + 1]
                i += 1
            i += 1
            continue
        if "=" in part:
            k, _, v = part.partition("=")
            out[k] = v
        i += 1
    return out


def outside_construct(detail: str) -> str:
    detail = detail.strip()
    if not detail:
        return "-"
    return detail.split(":", 1)[0].strip()


def aggregate_routes(program_fields: list[dict[str, str]]) -> tuple[str, str]:
    """Return (route_verdict bucket, first_refusal construct name)."""
    routes: list[str] = []
    refusals: list[str] = []

    for f in program_fields:
        route = f.get("route", "-")
        routes.append(route)
        if route == "outside":
            refusals.append(outside_construct(f.get("outside_detail", "")))
        elif route.startswith("outside"):
            refusals.append(outside_construct(route.split(":", 1)[-1]))

    first_refusal = refusals[0] if refusals else "-"

    has = lambda name: any(r == name or r.startswith(name + ",") for r in routes)
    first = lambda name: next((r for r in routes if r == name or r.startswith(name)), None)

    for decided in ("rejected",):
        if has(decided):
            return "rejected", first_refusal
    if has("incomplete"):
        return "outside", first_refusal

    for undecided in ("fallback", "refused-module", "defect", "panicked"):
        if first(undecided):
            if undecided == "fallback":
                return "outside", first_refusal
            if undecided in ("defect", "panicked"):
                return "error", first_refusal
            return "outside", first_refusal

    if has("accepted"):
        if first("outside") or any("outside" in r for r in routes):
            return "accepted", first_refusal
        return "accepted", first_refusal

    if has("no-entry"):
        if any(r not in ("no-entry", "-") and not r.startswith("unreached") for r in routes):
            pass
        else:
            return "outside", first_refusal or "no-entry"

    outside = first("outside") or next((r for r in routes if "outside" in r), None)
    if outside:
        if first_refusal == "-":
            first_refusal = outside_construct(outside)
        return "outside", first_refusal

    if has("unreached"):
        return "error", first_refusal

    if routes:
        r0 = routes[0]
        if r0 in ("no-entry",):
            return "outside", "no-entry"
        return "error", first_refusal

    return "error", first_refusal


def parse_harness_output(text: str) -> tuple[str, str, str, str]:
    program_fields: list[dict[str, str]] = []
    current = "-"
    fallback_construct = ""

    for line in text.splitlines():
        if not line.strip():
            continue
        parts = line.split("\t")
        kind = parts[0]
        if kind == "program" and len(parts) >= 3:
            f = field_map(parts[2:])
            if "current" in f and current == "-":
                current = f["current"]
            program_fields.append(f)
        elif kind == "fallback" and len(parts) >= 5:
            fallback_construct = parts[4]

    route_verdict, first_refusal = aggregate_routes(program_fields)
    if fallback_construct and first_refusal in ("-", ""):
        first_refusal = fallback_construct.strip()

    for line in text.splitlines():
        if line.startswith("fallback\t"):
            parts = line.split("\t")
            if len(parts) >= 5 and first_refusal in ("-", ""):
                first_refusal = parts[4].strip()
                break

    if route_verdict in ("rejected", "accepted"):
        first_refusal = "-"

    agreement = "n-a"
    if current in ("accepted", "rejected") and route_verdict in ("accepted", "rejected"):
        if current == route_verdict:
            agreement = "agree"
        else:
            agreement = "disagree"

    return route_verdict, current, agreement, first_refusal


def done_fixtures() -> set[str]:
    if not TSV.is_file():
        return set()
    done = set()
    for line in TSV.read_text().splitlines()[1:]:
        if line.strip():
            done.add(line.split("\t", 1)[0])
    return done


def run_one(fixture: str) -> str:
    tier = tier_of(fixture)
    cmd = [str(HARNESS)]
    if tier == "fail":
        cmd.append("--every-machine")
    cmd.append(fixture)
    exact = (
        f"timeout {TIMEOUT_S} {HARNESS}"
        + (" --every-machine" if tier == "fail" else "")
        + f" {fixture} < /dev/null"
    )
    started = time.monotonic()
    try:
        proc = subprocess.run(
            cmd,
            cwd=REPO,
            capture_output=True,
            text=True,
            timeout=TIMEOUT_S,
            stdin=subprocess.DEVNULL,
        )
        elapsed = time.monotonic() - started
        out = proc.stdout + proc.stderr
        if "program\t" not in out and "panicked" in out:
            return (
                f"{fixture}\t{tier}\terror\t-\tn-a\tpanicked\t{elapsed:.2f}\n",
                exact,
            )
        route_verdict, current, agreement, first_refusal = parse_harness_output(out)
        row = (
            f"{fixture}\t{tier}\t{route_verdict}\t{current}\t{agreement}\t"
            f"{first_refusal}\t{elapsed:.2f}\n"
        )
        return row, exact
    except subprocess.TimeoutExpired:
        elapsed = float(TIMEOUT_S)
        return (
            f"{fixture}\t{tier}\ttimeout\t-\tn-a\t-\t{elapsed:.2f}\n",
            exact,
        )


def main() -> None:
    fixtures = shard_fixtures(SHARD)
    done = done_fixtures()
    if not TSV.exists():
        TSV.write_text(HEADER)
    commands_logged: list[str] = []
    for i, fixture in enumerate(fixtures):
        if fixture in done:
            continue
        row, exact = run_one(fixture)
        with TSV.open("a") as f:
            f.write(row)
        commands_logged.append(exact)
        if (i + 1) % 25 == 0:
            print(f"progress {i + 1}/{len(fixtures)}", flush=True)
    md = OUT_DIR / f"shard-{SHARD}.md"
    harness_commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=REPO, text=True
    ).strip()
    pin = "0835b3aaf803ed97f905be42d94727ff58459263"
    body = [
        f"# Route coverage shard {SHARD}",
        "",
        f"PIN: `{pin}`",
        f"Harness built at: `{harness_commit}`",
        f"Fixtures in shard: {len(fixtures)}",
        f"Completed rows: {len(done_fixtures())}",
        "",
        "## Commands",
        "",
        "Build:",
        "```",
        f"cd {REPO}",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target "
        "mbx build -p compiler --example route_one_comparison < /dev/null",
        "```",
        "",
        "Per fixture (fail/ adds `--every-machine`):",
        "```",
        f"timeout {TIMEOUT_S} target/debug/examples/route_one_comparison "
        "[--every-machine] <fixture>/main.omg < /dev/null",
        "```",
        "",
    ]
    md.write_text("\n".join(body))


if __name__ == "__main__":
    main()
