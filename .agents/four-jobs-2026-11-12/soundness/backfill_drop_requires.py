#!/usr/bin/env python3
"""Generate and run missing drop_requires rows for fixtures already in shard progress."""

from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run_shard_mutations import (  # noqa: E402
    append_tsv,
    classify,
    load_progress,
    run_both_checkers,
    slug,
    stage_mutant,
)
from requires_span import drop_first_requires  # noqa: E402


def fixtures_missing_drop(tsv: Path, progress: set[str]) -> list[str]:
    have: set[str] = set()
    for line in tsv.read_text().splitlines()[1:]:
        parts = line.split("\t")
        if len(parts) >= 2 and parts[1] == "drop_requires":
            have.add(parts[0])
    return sorted(f for f in progress if f not in have)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--shard", type=int, required=True)
    ap.add_argument("--timeout", type=int, default=120)
    args = ap.parse_args()

    out = REPO / ".agents/four-jobs-2026-11-12/soundness"
    tsv = out / f"shard-{args.shard}.tsv"
    progress_path = out / f"shard-{args.shard}.progress"
    mut_root = out / f"shard-{args.shard}" / "mutants"
    progress = load_progress(progress_path)
    missing = fixtures_missing_drop(tsv, progress)
    added = 0
    for rel in missing:
        main = REPO / rel
        if not main.exists():
            continue
        source = main.read_text(encoding="utf-8")
        mutated = drop_first_requires(source)
        if not mutated or mutated == source:
            continue
        mdir = mut_root / slug(rel) / "drop_requires"
        try:
            stage_mutant(main, mdir, mutated)
        except OSError:
            continue
        mrel = str((mdir / "main.omg").relative_to(REPO))
        current, route, route_reason = run_both_checkers(mdir / "main.omg", args.timeout)
        hand, spec_path, spec_quote, obligation, review_reason, acc_by, hole_subtype = classify(
            "drop_requires", source, mutated, current, route, route_reason
        )
        if hand not in {"confirmed-hole", "known-gap-q7-mut"}:
            shutil.rmtree(mdir, ignore_errors=True)
        append_tsv(
            tsv,
            [
                rel,
                "drop_requires",
                mrel,
                current,
                route,
                hand,
                spec_path,
                spec_quote.replace("\t", " ").replace("\n", " "),
                obligation.replace("\t", " "),
                route_reason.replace("\t", " ")[:500],
                review_reason.replace("\t", " ")[:500],
                acc_by,
                hole_subtype,
            ],
        )
        added += 1
        if added % 20 == 0:
            print(f"added {added} drop_requires rows…", flush=True)
    print(f"backfill complete: {added} drop_requires rows for {len(missing)} fixtures checked")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
