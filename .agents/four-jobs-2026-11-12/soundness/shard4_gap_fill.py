#!/usr/bin/env python3
"""Re-attempt mutations on accepted fixtures that have no mutant dirs (18 gaps)."""

from __future__ import annotations

import csv
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
PROGRESS = OUT / "shard-4.progress"


def gaps() -> list[Path]:
    rows = list(csv.DictReader((OUT / "shard-4.tsv").open(), delimiter="\t"))
    orig = {r["fixture"]: r for r in rows if r["stage"] == "original"}
    mut_fix = {r["fixture"] for r in rows if r["stage"] == "mutant" and r.get("mutant_path")}
    return [REPO / f for f, r in orig.items() if r.get("current_verdict") == "accepted" and f not in mut_fix]


def clear_progress(prefix: str) -> None:
    if not PROGRESS.exists():
        return
    kept = [
        ln
        for ln in PROGRESS.read_text().splitlines()
        if ln.strip() and not ln.startswith(prefix)
    ]
    PROGRESS.write_text("\n".join(kept) + ("\n" if kept else ""))


def main() -> int:
    fx = gaps()
    if not fx:
        print("no gaps")
        return 0
    for path in fx:
        rel = path.relative_to(REPO).as_posix()
        clear_progress(f"fixture:{rel}")
    runner = OUT / "shard4_runner.py"
    for path in fx:
        rel = path.relative_to(REPO).as_posix()
        env = {**os.environ, "SKIP_BASELINE_LOG": "1"}
        subprocess.run(
            [sys.executable, str(runner), "--fixture", rel],
            cwd=REPO,
            check=False,
            env=env,
        )
    print(f"attempted gap fill for {len(fx)} fixtures — re-run reclassify + bucket_fix + finalize")
    return 0


if __name__ == "__main__":
    sys.exit(main())
