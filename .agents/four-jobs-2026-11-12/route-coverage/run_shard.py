#!/usr/bin/env python3
"""Run route_one_comparison for one coverage-map shard and append shard-S.tsv."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 4
SHARD_MOD = 8
LIST_SCRIPT = (
    "git ls-files 'tests/omega/pass/**/main.omg' 'tests/omega/run/**/main.omg' "
    "'tests/omega/fail/**/main.omg' | LC_ALL=C sort"
)
HARNESS = REPO / "target-o3/debug/examples/route_one_comparison"
OUT_DIR = REPO / ".agents/four-jobs-2026-11-12/route-coverage"
TSV = OUT_DIR / f"shard-{SHARD}.tsv"
PARSER = OUT_DIR / "run_shard_fixture.py"
TIMEOUT_S = 300


def shard_fixtures() -> list[str]:
    proc = subprocess.run(
        LIST_SCRIPT,
        shell=True,
        cwd=REPO,
        check=True,
        capture_output=True,
        text=True,
    )
    fixtures: list[str] = []
    for index, line in enumerate(proc.stdout.splitlines()):
        if index % SHARD_MOD == SHARD:
            fixtures.append(line.strip())
    return fixtures


def load_done() -> set[str]:
    if not TSV.exists():
        return set()
    done: set[str] = set()
    for line in TSV.read_text(encoding="utf-8").splitlines():
        if line.startswith("fixture\t"):
            continue
        if line.strip():
            done.add(line.split("\t", 1)[0])
    return done


def run_one(fixture: str) -> tuple[str, float]:
    main = REPO / fixture
    every = "--every-machine" if "/fail/" in fixture else ""
    cmd = [
        "timeout",
        str(TIMEOUT_S),
        str(HARNESS),
    ]
    if every:
        cmd.append(every)
    cmd.append(str(main))
    started = time.monotonic()
    try:
        proc = subprocess.run(
            cmd,
            cwd=REPO,
            stdin=subprocess.DEVNULL,
            capture_output=True,
            text=True,
        )
        elapsed = time.monotonic() - started
        if proc.returncode == 124:
            tier = "pass" if "/pass/" in fixture else "run" if "/run/" in fixture else "fail"
            if proc.stdout.strip():
                tmp = OUT_DIR / ".last_harness.out"
                tmp.write_text(proc.stdout, encoding="utf-8")
                try:
                    row_proc = subprocess.run(
                        [sys.executable, str(PARSER), fixture, str(tmp), str(elapsed)],
                        check=True,
                        capture_output=True,
                        text=True,
                    )
                    parts = row_proc.stdout.strip().split("\t")
                    parts[2] = "timeout"
                    parts[4] = "n-a"
                    return "\t".join(parts), elapsed
                except subprocess.CalledProcessError:
                    pass
            return (
                f"{fixture}\t{tier}\ttimeout\t?\tn-a\t\t{elapsed:.3f}",
                elapsed,
            )
        if proc.returncode != 0 and not proc.stdout.strip():
            tier = "pass" if "/pass/" in fixture else "run" if "/run/" in fixture else "fail"
            detail = (proc.stderr or "harness failed").replace("\t", " ").replace("\n", " ")[:200]
            return (
                f"{fixture}\t{tier}\terror\t?\tn-a\t{detail}\t{elapsed:.3f}",
                elapsed,
            )
        tmp = OUT_DIR / ".last_harness.out"
        tmp.write_text(proc.stdout, encoding="utf-8")
        row_proc = subprocess.run(
            [sys.executable, str(PARSER), fixture, str(tmp), str(elapsed)],
            check=True,
            capture_output=True,
            text=True,
        )
        return row_proc.stdout.strip(), elapsed
    except subprocess.TimeoutExpired:
        elapsed = time.monotonic() - started
        tier = "pass" if "/pass/" in fixture else "run" if "/run/" in fixture else "fail"
        return (
            f"{fixture}\t{tier}\ttimeout\t?\tn-a\t\t{elapsed:.3f}",
            elapsed,
        )


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    fixtures = shard_fixtures()
    done = load_done()
    if not TSV.exists():
        TSV.write_text(
            "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\tfirst_refusal\tseconds\n",
            encoding="utf-8",
        )
    remaining = [f for f in fixtures if f not in done]
    print(f"shard {SHARD}: {len(fixtures)} total, {len(done)} done, {len(remaining)} remaining")
    for index, fixture in enumerate(remaining, start=1):
        row, elapsed = run_one(fixture)
        with TSV.open("a", encoding="utf-8") as handle:
            handle.write(row + "\n")
        print(f"[{index}/{len(remaining)}] {fixture} {elapsed:.1f}s -> {row.split(chr(9))[2]}")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
