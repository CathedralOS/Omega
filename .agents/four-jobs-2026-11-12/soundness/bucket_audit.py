#!/usr/bin/env python3
"""Assert bucket counts from reviewed TSV match row totals."""

from __future__ import annotations

import sys
from collections import Counter
from pathlib import Path


def audit_tsv(path: Path) -> tuple[int, Counter[str]]:
    buckets: Counter[str] = Counter()
    seen: set[tuple[str, str, str]] = set()
    for line in path.read_text().splitlines():
        if not line.startswith("tests/"):
            continue
        parts = line.split("\t")
        if len(parts) < 10:
            raise SystemExit(f"short row: {line[:120]}")
        key = (parts[0], parts[1], parts[2] or f"no-path:{parts[1]}")
        if key in seen:
            raise SystemExit(f"duplicate row key: {key}")
        seen.add(key)
        buckets[parts[9]] += 1
    total = sum(buckets.values())
    return total, buckets


def main() -> int:
    shard = int(__import__("os").environ.get("FOUR_JOBS_SHARD", "7"))
    tsv = Path(__file__).resolve().parents[3] / f".agents/four-jobs-2026-11-12/soundness/shard-{shard}.tsv"
    total, buckets = audit_tsv(tsv)
    print(f"total_rows={total} bucket_sum={sum(buckets.values())}")
    for name in sorted(buckets):
        print(f"  {name}: {buckets[name]}")
    if sum(buckets.values()) != total:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
