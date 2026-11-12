#!/usr/bin/env python3
"""Run route_one_comparison on shard fixtures; append TSV rows with resume."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
import time
from pathlib import Path

_DIR = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("parse_route_output", _DIR / "parse_route_output.py")
_parse = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_parse)

REPO = _DIR.parents[2]
HARNESS = REPO / "target/debug/examples/route_one_comparison"
OUT_DIR = _DIR
SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 1
TSV = OUT_DIR / f"shard-{SHARD}.tsv"
TIMEOUT_S = 300
MODE = sys.argv[2] if len(sys.argv) > 2 else "resume"

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


def done_fixtures() -> set[str]:
    if not TSV.is_file():
        return set()
    done = set()
    for line in TSV.read_text().splitlines()[1:]:
        if line.strip():
            done.add(line.split("\t", 1)[0])
    return done


def run_one(fixture: str) -> tuple[str, str]:
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
            row = f"{fixture}\t{tier}\terror\t-\tn-a\tmeta:harness-panicked\t{elapsed:.2f}\n"
            return row, exact
        route_verdict, current, agreement, first_refusal = _parse.parse_harness_output(
            out
        )
        row = (
            f"{fixture}\t{tier}\t{route_verdict}\t{current}\t{agreement}\t"
            f"{first_refusal}\t{elapsed:.2f}\n"
        )
        return row, exact
    except subprocess.TimeoutExpired:
        elapsed = float(TIMEOUT_S)
        row = f"{fixture}\t{tier}\ttimeout\t-\tn-a\t-\t{elapsed:.2f}\n"
        return row, exact


def main() -> None:
    fixtures = shard_fixtures(SHARD)
    if MODE == "rebuild":
        TSV.write_text(HEADER)
        done: set[str] = set()
    else:
        done = done_fixtures()
        if not TSV.exists():
            TSV.write_text(HEADER)

    for i, fixture in enumerate(fixtures):
        if fixture in done:
            continue
        row, _exact = run_one(fixture)
        with TSV.open("a") as f:
            f.write(row)
        if (i + 1) % 25 == 0:
            print(f"progress {i + 1}/{len(fixtures)}", flush=True)

    print(f"finished {len(done_fixtures())}/{len(fixtures)} rows", flush=True)


if __name__ == "__main__":
    main()
