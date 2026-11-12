#!/usr/bin/env python3
"""Rebuild shard-1.tsv from checkpoint + mutant dirs (after TSV corruption)."""
from __future__ import annotations

import re
import sys
from pathlib import Path

_HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(_HERE))
import run_shard1 as r1

REPO = Path(__file__).resolve().parents[3]
OUT = _HERE
TSV = OUT / "shard-1.tsv"
CHECKPOINT = OUT / "shard-1.checkpoint"
MUT_ROOT = OUT / "shard-1"


def find_mutant(fixture: str) -> tuple[str, Path] | None:
    slug = r1.slug(fixture)
    for p in sorted(MUT_ROOT.iterdir()):
        if not p.is_dir() or p.name.startswith("_staging"):
            continue
        if not p.name.startswith(slug):
            continue
        kind = p.name.split("__", 1)[-1] if "__" in p.name else "unknown"
        return kind, p
    return None


def main() -> None:
    fixtures = r1.fixture_list()
    done = {ln.strip() for ln in CHECKPOINT.read_text().splitlines() if ln.strip()}
    assert len(done) >= len(fixtures) - 5, (len(done), len(fixtures))

    with TSV.open("w", encoding="utf-8") as tsv:
        tsv.write(
            "fixture\ttier\tmutation_kind\tmutant_path\tcurrent_verdict\t"
            "route_verdict\troute_reason\tagree\thand_verdict\tverdict_reason\t"
            "spec_file\tspec_quote\tobligation_line\tcurrent_out\troute_out\t"
            "current_s\troute_s\n"
        )
        for fixture in fixtures:
            if fixture not in done:
                tsv.write(
                    f"{fixture}\t{r1.tier_of(fixture)}\tnone\t\trejected\trejected\t"
                    f"pending\tdisagree\tfailed-run\tfixture not in checkpoint\t"
                    f"\t\t\t\t\t0\t0\n"
                )
                continue
            hit = find_mutant(fixture)
            if not hit:
                tsv.write(
                    f"{fixture}\t{r1.tier_of(fixture)}\tnone\t\trejected\trejected\t"
                    f"no dir\tagree\tno-mutation\tmissing mutant dir\t"
                    f"\t\t\t\t\t0\t0\n"
                )
                continue
            kind, mdir = hit
            main = mdir / "main.omg"
            cur, cs, c_out = r1.run_omega(main)
            rt, rs, r_out, r_reason = r1.run_route(main)
            agree = "agree" if cur == rt else "disagree"
            rel = str(mdir.relative_to(REPO))
            tsv.write(
                f"{fixture}\t{r1.tier_of(fixture)}\t{kind}\t{rel}\t{cur}\t{rt}\t"
                f"{r_reason.replace(chr(9),' ').replace(chr(10),' ')}\t{agree}\tpending-triage\t\t"
                f"\t\t\t{c_out.replace(chr(9),' ').replace(chr(10),' ')}\t"
                f"{r_out.replace(chr(9),' ').replace(chr(10),' ')}\t{cs:.3f}\t{rs:.3f}\n"
            )
    print("wrote", sum(1 for _ in open(TSV)) - 1, "rows")


if __name__ == "__main__":
    main()
