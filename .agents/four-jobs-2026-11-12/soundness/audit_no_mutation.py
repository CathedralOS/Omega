#!/usr/bin/env python3
"""Explain why fixtures produced _no_mutations rows."""

from __future__ import annotations

import argparse
import sys
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).resolve().parent))
from requires_span import drop_first_requires  # noqa: E402
from run_shard_mutations import (  # noqa: E402
    mut_amp_to_ampmut,
    mut_ampmut_to_amp,
    mut_break_ensures_zero,
    mut_drop_match_guard_division,
    mut_remove_let_mut,
    mut_widen_requires_nonzero,
    mut_force_write_before_borrow_end,
)

GENERATORS = {
    "drop_requires": drop_first_requires,
    "widen_requires": mut_widen_requires_nonzero,
    "drop_divisor_guard": mut_drop_match_guard_division,
    "remove_mut": mut_remove_let_mut,
    "flip_borrow_mutability_ampmut": mut_amp_to_ampmut,
    "flip_borrow_mutability_amp": mut_ampmut_to_amp,
    "write_through_shared_borrow": mut_force_write_before_borrow_end,
    "break_ensures": mut_break_ensures_zero,
}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--shard", type=int, required=True)
    args = ap.parse_args()
    tsv = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{args.shard}.tsv"
    no_mut: list[str] = []
    for line in tsv.read_text().splitlines()[1:]:
        parts = line.split("\t")
        if len(parts) > 1 and parts[1] == "_no_mutations":
            no_mut.append(parts[0])
    reasons: Counter[str] = Counter()
    for rel in no_mut:
        main = REPO / rel
        if not main.exists():
            reasons["fixture-missing"] += 1
            continue
        text = main.read_text(encoding="utf-8")
        produced = 0
        for name, fn in GENERATORS.items():
            out = fn(text)
            if out is not None and out != text:
                produced += 1
            else:
                reasons[name] += 1
        if produced == 0:
            reasons["all-generators-inert"] += 1
    print(f"no_mutation fixtures: {len(no_mut)}")
    for name, count in reasons.most_common():
        print(f"  {name}: {count}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
