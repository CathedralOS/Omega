#!/usr/bin/env python3
"""Re-parse shard-0 TSV from saved harness output under route-coverage/raw/."""

from __future__ import annotations

import sys
from pathlib import Path

# Reuse parser from run_shard_0 in the same directory.
import run_shard_0 as shard

REPO = Path(__file__).resolve().parents[3]
OUT_DIR = REPO / ".agents/four-jobs-2026-11-12/route-coverage"
LEGACY_SECONDS = REPO / ".agents/four-jobs-2026-11-12/1/shard-0.tsv"


def load_legacy_seconds() -> dict[str, float]:
    if not LEGACY_SECONDS.is_file():
        return {}
    out: dict[str, float] = {}
    for line in LEGACY_SECONDS.read_text(encoding="utf-8").splitlines()[1:]:
        if not line.strip():
            continue
        parts = line.split("\t")
        if len(parts) >= 7:
            try:
                out[parts[0]] = float(parts[-1])
            except ValueError:
                pass
    return out


def main() -> int:
    tsv_path = OUT_DIR / f"shard-{shard.SHARD}.tsv"
    md_path = OUT_DIR / f"shard-{shard.SHARD}.md"
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    legacy_seconds = load_legacy_seconds()

    fixtures = shard.fixture_list()
    missing: list[str] = []
    rows_out: list[str] = []
    for index, fixture in enumerate(fixtures, start=1):
        raw = shard.raw_path(fixture)
        if raw.is_file():
            text = raw.read_text(encoding="utf-8")
            seconds = legacy_seconds.get(fixture, 0.0)
            row = shard.parse_output(fixture, text, seconds, False, 0)
            rows_out.append(row)
        else:
            missing.append(fixture)
        if index % 200 == 0:
            print(f"parsed {index}/{len(fixtures)}", flush=True)

    if missing:
        print(
            f"missing raw for {len(missing)} fixtures; run harness to fill raw/",
            file=sys.stderr,
        )
        for fixture in missing:
            text, seconds, timed_out, exit_code = shard.run_harness(fixture)
            shard.raw_path(fixture).write_text(text, encoding="utf-8")
            rows_out.append(
                shard.parse_output(fixture, text, seconds, timed_out, exit_code)
            )
            if len(missing) <= 50 or missing.index(fixture) % 25 == 0:
                print(f"harness {fixture}", flush=True)

    tsv_path.write_text(shard.TSV_HEADER + "".join(rows_out), encoding="utf-8")
    shard.write_summary_md(tsv_path, md_path)
    print(f"wrote {len(rows_out)} rows", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
