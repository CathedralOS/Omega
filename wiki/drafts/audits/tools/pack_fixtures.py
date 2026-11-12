#!/usr/bin/env python3
"""Pack fixture paths into filter batches without substring collisions (max 200 each)."""
from __future__ import annotations

import argparse
from pathlib import Path


def can_add(batch: list[str], item: str) -> bool:
    for other in batch:
        if item in other or other in item:
            return False
    return True


def pack(fixtures: list[str], max_size: int = 200) -> list[list[str]]:
    fixtures = sorted(fixtures, key=len, reverse=True)
    batches: list[list[str]] = []
    for fix in fixtures:
        placed = False
        for batch in batches:
            if len(batch) < max_size and can_add(batch, fix):
                batch.append(fix)
                placed = True
                break
        if not placed:
            batches.append([fix])
    return batches


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("fixtures_file", type=Path)
    ap.add_argument("out_dir", type=Path)
    ap.add_argument("--max", type=int, default=200)
    args = ap.parse_args()
    fixtures = [ln.strip() for ln in args.fixtures_file.read_text().splitlines() if ln.strip()]
    batches = pack(fixtures, args.max)
    args.out_dir.mkdir(parents=True, exist_ok=True)
    manifest = []
    for i, batch in enumerate(batches):
        p = args.out_dir / f"batch_{i}.filter"
        p.write_text(",".join(batch))
        manifest.append({"id": i, "count": len(batch), "filter": str(p.name)})
    (args.out_dir / "manifest.json").write_text(__import__("json").dumps(manifest, indent=2))
    print(f"{len(fixtures)} fixtures -> {len(batches)} batches sizes={[len(b) for b in batches]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
