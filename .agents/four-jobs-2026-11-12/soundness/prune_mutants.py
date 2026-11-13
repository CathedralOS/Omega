#!/usr/bin/env python3
"""Move non-kept mutants to /tmp and drop them from git."""

from __future__ import annotations

import csv
import os
import shutil
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SHARD = int(os.environ.get("SOUNDNESS_SHARD", "6"))
HERE = REPO / ".agents" / "four-jobs-2026-11-12" / "soundness"
TSV = HERE / f"shard-{SHARD}.tsv"
MUTANTS = HERE / f"shard-{SHARD}" / "mutants"
TMP = Path(f"/tmp/soundness-shard-{SHARD}-mutants-pruned")


def main() -> int:
    keep_dirs: set[str] = set()
    with TSV.open(encoding="utf-8") as f:
        for row in csv.DictReader(f, delimiter="\t"):
            if row["bucket"] not in ("confirmed-hole", "known-gap-q7-mut", "disagreement-valid"):
                continue
            path = row.get("mutant_path", "").strip()
            if not path or not path.startswith(".agents/"):
                continue
            keep_dirs.add(Path(path).parent.name)
    TMP.mkdir(parents=True, exist_ok=True)
    if not MUTANTS.is_dir():
        return 0
    removed = kept = 0
    for child in MUTANTS.iterdir():
        if not child.is_dir():
            continue
        if child.name in keep_dirs:
            kept += 1
            continue
        dest = TMP / child.name
        if dest.exists():
            shutil.rmtree(dest)
        shutil.move(str(child), str(dest))
        removed += 1
    subprocess.run(
        ["git", "rm", "-r", "--cached", f".agents/four-jobs-2026-11-12/soundness/shard-{SHARD}/mutants"],
        cwd=REPO,
        capture_output=True,
    )
    for name in keep_dirs:
        rel = f".agents/four-jobs-2026-11-12/soundness/shard-{SHARD}/mutants/{name}"
        if (REPO / rel).is_dir():
            subprocess.run(["git", "add", "-f", rel], cwd=REPO, capture_output=True)
    print(f"kept {kept} mutant dirs, moved {removed} to {TMP}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
