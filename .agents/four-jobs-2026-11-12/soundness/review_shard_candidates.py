#!/usr/bin/env python3
"""Turn TSV candidates into shard markdown with wiki/spec quotes by mutation kind."""

from __future__ import annotations

import re
import sys
from pathlib import Path

_SCRIPT_DIR = Path(__file__).resolve().parent
if str(_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPT_DIR))

from spec_quotes import SPEC_QUOTES

REPO = Path(__file__).resolve().parents[3]
SHARD = 7
OUT_ROOT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT_ROOT / f"shard-{SHARD}.tsv"
MD = OUT_ROOT / f"shard-{SHARD}.md"

def confirms_invalid(kind: str, src: str) -> bool:
    """Heuristic hand-check: mutation must break the cited rule, not merely weaken syntax."""
    if kind == "drop_mut":
        return "let mut" not in src and ("=" in src or "&mut" in src)
    if kind == "shared_instead_of_mut":
        return "&mut" not in src and "= " in src
    if kind == "mut_ref_instead_of_shared":
        return "&mut" in src
    if kind in ("drop_requires", "widen_requires", "break_ensures"):
        return True
    if kind == "drop_ensures":
        return False
    if kind == "drop_guard":
        return "if " not in src and ("/" in src or "[" in src)
    return True


def route_accepts(route: str) -> bool:
    return route == "accepted"


def checker_accepts(verdict: str) -> bool:
    return verdict == "accepted"


def main() -> None:
    if not TSV.exists():
        MD.write_text(f"# Soundness mutations shard {SHARD}\n\n(no data yet)\n")
        return

    rows = []
    for line in TSV.read_text().splitlines():
        if not line or line.startswith("#") or line.startswith("original_fixture"):
            continue
        parts = line.split("\t")
        if len(parts) < 9:
            continue
        if parts[-1] != "yes":
            continue
        rows.append(parts)

    kept_holes = []
    disagreements = []
    discarded_valid = 0
    discarded_no_rule = 0

    for parts in rows:
        orig, kind, mutant, omg, cur, route, secs, _cand = parts[:8]
        src = (REPO / mutant).read_text()
        spec_entry = SPEC_QUOTES.get(kind)
        if not spec_entry:
            discarded_no_rule += 1
            continue
        spec_path, quote = spec_entry

        any_accept = (
            checker_accepts(omg)
            or checker_accepts(cur)
            or route_accepts(route)
        )
        disagree = (
            omg in ("accepted", "rejected")
            and cur in ("accepted", "rejected")
            and omg != "skipped"
            and omg != cur
        ) or (
            cur in ("accepted", "rejected")
            and route in ("accepted", "rejected", "incomplete", "fallback")
            and cur != route
        )

        if not any_accept and not disagree:
            continue

        if not confirms_invalid(kind, src):
            discarded_valid += 1
            continue

        hand = "confirmed-invalid"
        if any_accept:
            kept_holes.append((parts, spec_path, quote, hand))
        elif disagree:
            disagreements.append((parts, spec_path, quote, hand))

    generated = run = kept = 0
    counts_file = OUT_ROOT / f"shard-{SHARD}.counts"
    if counts_file.exists():
        for ln in counts_file.read_text().splitlines():
            if "=" in ln:
                k, v = ln.split("=", 1)
                if k == "generated":
                    generated = int(v)
                elif k == "run":
                    run = int(v)
                elif k == "candidate":
                    pass

    lines = [
        f"# Soundness mutations shard {SHARD} (job 2)",
        "",
        "PIN: `0835b3aaf803ed97f905be42d94727ff58459263`",
        "",
        "## Commands",
        "",
        "```bash",
        "cd /agent/repos/omega",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison",
        "FOUR_JOBS_SHARD=7 python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py",
        "python3 .agents/four-jobs-2026-11-12/soundness/review_shard_candidates.py",
        "```",
        "",
        "Per mutant: `timeout 120 omega --check <mutant>/main.omg` and",
        "`timeout 120 target/debug/examples/route_one_comparison <mutant>/main.omg`.",
        "",
        "## Counts",
        "",
        f"- mutants generated (this run): {generated}",
        f"- mutants run: {run}",
        f"- candidates flagged in TSV: {len(rows)}",
        f"- confirmed holes in report: {len(kept_holes)}",
        f"- checker disagreements: {len(disagreements)}",
        f"- discarded (no spec quote): {discarded_no_rule}",
        f"- discarded (valid mutant): {discarded_valid}",
        "",
        "## Confirmed holes",
        "",
    ]

    for parts, spec_path, quote, hand in kept_holes:
        orig, kind, mutant, omg, cur, route, secs, _ = parts[:8]
        src = (REPO / mutant).read_text()
        lines.extend(
            [
                f"### {kind} — `{orig}`",
                "",
                f"- Mutant: `{mutant}`",
                f"- omega: **{omg}**; route current: **{cur}**; route: **{route}**",
                f"- Hand-check: **{hand}**",
                f"- Spec: `wiki/spec/{spec_path}`",
                "",
                "> " + quote.replace("\n", "\n> ") if quote else "(missing quote)",
                "",
                "```omg",
                src.strip(),
                "```",
                "",
            ]
        )

    lines.extend(["## Disagreements", ""])
    for parts, spec_path, quote, hand in disagreements:
        orig, kind, mutant, omg, cur, route, secs, _ = parts[:8]
        lines.extend(
            [
                f"### {kind} — `{orig}`",
                "",
                f"- Mutant: `{mutant}`",
                f"- omega: **{omg}**; route current: **{cur}**; route: **{route}**",
                "",
            ]
        )
        if quote and spec_path:
            lines.extend(
                [
                    f"- Spec (invalidity): `wiki/spec/{spec_path}`",
                    "",
                    "> " + quote.replace("\n", "\n> "),
                    "",
                ]
            )

    MD.write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
