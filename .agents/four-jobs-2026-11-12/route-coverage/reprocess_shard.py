#!/usr/bin/env python3
"""Re-run harness for fixtures already listed in shard-S.tsv (parser refresh)."""

from __future__ import annotations

import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 4
OUT_DIR = REPO / ".agents/four-jobs-2026-11-12/route-coverage"
TSV = OUT_DIR / f"shard-{SHARD}.tsv"
BACKUP = OUT_DIR / f"shard-{SHARD}.pre-fix.tsv"
HARNESS = REPO / "target-o3/debug/examples/route_one_comparison"
PARSER = OUT_DIR / "run_shard_fixture.py"
TIMEOUT_S = 300


def fixtures_from_tsv() -> list[str]:
    lines = TSV.read_text(encoding="utf-8").splitlines()
    return [line.split("\t", 1)[0] for line in lines[1:] if line.strip()]


def run_one(fixture: str) -> str:
    main = REPO / fixture
    cmd = ["timeout", str(TIMEOUT_S), str(HARNESS)]
    if "/fail/" in fixture:
        cmd.append("--every-machine")
    cmd.append(str(main))
    started = time.monotonic()
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
        return f"{fixture}\t{tier}\ttimeout\t?\tn-a\t\t{elapsed:.3f}"
    if proc.returncode != 0 and not proc.stdout.strip():
        tier = "pass" if "/pass/" in fixture else "run" if "/run/" in fixture else "fail"
        detail = (proc.stderr or "harness failed").replace("\t", " ")[:200]
        return f"{fixture}\t{tier}\terror\t?\tn-a\tbucket:harness-failure:{detail}\t{elapsed:.3f}"
    tmp = OUT_DIR / ".last_harness.out"
    tmp.write_text(proc.stdout, encoding="utf-8")
    row_proc = subprocess.run(
        [sys.executable, str(PARSER), fixture, str(tmp), str(elapsed)],
        check=True,
        capture_output=True,
        text=True,
    )
    return row_proc.stdout.strip()


def main() -> None:
    if not TSV.exists():
        print("no tsv", file=sys.stderr)
        raise SystemExit(1)
    BACKUP.write_text(TSV.read_text(encoding="utf-8"), encoding="utf-8")
    fixtures = fixtures_from_tsv()
    header = "fixture\ttier\troute_verdict\tcurrent_verdict\tagreement\tfirst_refusal\tseconds\n"
    rows: list[str] = []
    for index, fixture in enumerate(fixtures, 1):
        row = run_one(fixture)
        rows.append(row)
        print(f"[reprocess {index}/{len(fixtures)}] {fixture} -> {row.split(chr(9))[2]}")
        sys.stdout.flush()
    TSV.write_text(header + "\n".join(rows) + "\n", encoding="utf-8")
    print(f"rewrote {TSV} ({len(rows)} rows), backup {BACKUP}")


if __name__ == "__main__":
    main()
