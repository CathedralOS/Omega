#!/usr/bin/env python3
"""Run all fixtures for shard S; resume from partial TSV."""

from __future__ import annotations

import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
JOB_DIR = Path(__file__).resolve().parent
SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 2
PARALLEL = int(sys.argv[2]) if len(sys.argv) > 2 else 3
OUT = JOB_DIR / f"shard-{SHARD}.tsv"
RUNNER = JOB_DIR / "run_shard_fixture.py"
HEADER = (
    "fixture\ttier\tprogram_verdict\tcurrent_verdict\tagreement\t"
    "first_refusal\tseconds\troots\tvacuous_accept_roots\n"
)


def shard_fixtures() -> list[str]:
    proc = subprocess.run(
        "git ls-files 'tests/omega/pass/**/main.omg' 'tests/omega/run/**/main.omg' "
        "'tests/omega/fail/**/main.omg' | LC_ALL=C sort",
        shell=True,
        cwd=str(REPO),
        capture_output=True,
        text=True,
        check=True,
    )
    lines = [ln for ln in proc.stdout.splitlines() if ln.strip()]
    return [line for i, line in enumerate(lines) if i % 8 == SHARD]


def load_done() -> set[str]:
    if not OUT.exists():
        return set()
    done: set[str] = set()
    for line in OUT.read_text().splitlines()[1:]:
        if line.strip():
            done.add(line.split("\t", 1)[0])
    return done


def run_one(fixture: str) -> str:
    proc = subprocess.run(
        [sys.executable, str(RUNNER), fixture],
        capture_output=True,
        text=True,
        cwd=str(REPO),
    )
    lines = [ln for ln in proc.stdout.strip().splitlines() if ln.strip()]
    return lines[-1] if lines else f"{fixture}\t?\t?\t?\t?\t?\t0\t\t"


def main() -> None:
    fixtures = shard_fixtures()
    done = load_done()
    todo = [f for f in fixtures if f not in done]
    if not OUT.exists():
        OUT.write_text(HEADER)
    print(
        f"shard {SHARD}: {len(fixtures)} total, {len(done)} done, {len(todo)} todo",
        flush=True,
    )
    with ThreadPoolExecutor(max_workers=PARALLEL) as pool:
        futures = {pool.submit(run_one, f): f for f in todo}
        completed = 0
        for fut in as_completed(futures):
            line = fut.result()
            with OUT.open("a") as out:
                out.write(line + "\n")
            completed += 1
            if completed % 25 == 0 or completed == len(todo):
                print(f"progress {completed}/{len(todo)}", flush=True)


if __name__ == "__main__":
    main()
