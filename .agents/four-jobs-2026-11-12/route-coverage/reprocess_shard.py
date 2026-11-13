#!/usr/bin/env python3
"""Re-run harness for fixtures listed in shard-S.tsv (parser refresh)."""

from __future__ import annotations

import hashlib
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

REPO = Path(__file__).resolve().parents[3]
SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 4
OUT_DIR = REPO / ".agents/four-jobs-2026-11-12/route-coverage"
TSV = OUT_DIR / f"shard-{SHARD}.tsv"
RAW_DIR = Path(f"/tmp/route-coverage-shard-{SHARD}")
HARNESS = REPO / "target-o3/debug/examples/route_one_comparison"
PARSER = OUT_DIR / "run_shard_fixture.py"
TIMEOUT_S = 300

HEADER = (
    "fixture\ttier\tprogram_verdict\tcurrent_verdict\tagreement\tfirst_refusal\t"
    "seconds\troots\tvacuous_accept_roots\n"
)


def fixtures_from_tsv() -> list[str]:
    lines = TSV.read_text(encoding="utf-8").splitlines()
    fixtures = [line.split("\t", 1)[0] for line in lines[1:] if line.strip()]
    seen: set[str] = set()
    unique: list[str] = []
    for fixture in fixtures:
        if fixture not in seen:
            seen.add(fixture)
            unique.append(fixture)
    return unique


def raw_path(fixture: str) -> Path:
    digest = hashlib.sha1(fixture.encode()).hexdigest()[:16]
    name = fixture.replace("/", "__")
    return RAW_DIR / f"{digest}__{name}.out"


def run_one(fixture: str) -> str:
    tier = "pass" if "/pass/" in fixture else "run" if "/run/" in fixture else "fail"
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
    if proc.stdout.strip():
        RAW_DIR.mkdir(parents=True, exist_ok=True)
        raw_path(fixture).write_text(proc.stdout, encoding="utf-8")
    if proc.returncode == 124:
        from run_shard_fixture import row_from_timeout

        return "\t".join(row_from_timeout(fixture, elapsed, tier))
    if proc.returncode != 0 and not proc.stdout.strip():
        from run_shard_fixture import row_from_harness_failure

        detail = (proc.stderr or "harness failed").replace("\t", " ").replace("\n", " ")
        return "\t".join(row_from_harness_failure(fixture, elapsed, tier, detail))
    outfile = raw_path(fixture)
    row_proc = subprocess.run(
        [sys.executable, str(PARSER), fixture, str(outfile), str(elapsed)],
        check=True,
        capture_output=True,
        text=True,
    )
    return row_proc.stdout.strip()


def main() -> None:
    if not TSV.exists():
        print("no tsv", file=sys.stderr)
        raise SystemExit(1)
    fixtures = fixtures_from_tsv()
    rows: list[str] = []
    for index, fixture in enumerate(fixtures, 1):
        row = run_one(fixture)
        rows.append(row)
        verdict = row.split("\t", 3)[2]
        print(f"[reprocess {index}/{len(fixtures)}] {fixture} -> {verdict}")
        sys.stdout.flush()
    if len(rows) != len(fixtures):
        raise SystemExit(f"row count mismatch {len(rows)} vs {len(fixtures)}")
    TSV.write_text(HEADER + "\n".join(rows) + "\n", encoding="utf-8")
    print(f"rewrote {TSV} ({len(rows)} rows); raw harness under {RAW_DIR}")


if __name__ == "__main__":
    main()
