#!/usr/bin/env python3
"""Rewrite shard-S.md with bucket table and hand-verification sections."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).resolve().parent))
from run_shard_mutations import (  # noqa: E402
    PIN,
    _holes_from_tsv,
    _md_lines,
    _progress_count,
    _totals_from_tsv,
)
from run_shard_mutations import Counters  # noqa: E402


def capture(main: Path) -> str:
    import os

    route = REPO / "target/debug/examples/route_one_comparison"
    omega = REPO / "target/debug/omega"
    env = os.environ.copy()
    env["OMEGA_LIBRARY_ROOT"] = str(REPO / "source/library")
    env["OMEGA_CORPUS_ROOT"] = str(REPO)
    out: list[str] = []
    for label, cmd in (
        ("omega", [str(omega), "--check", str(main)]),
        ("route", [str(route), str(main)]),
    ):
        p = subprocess.run(
            cmd, capture_output=True, text=True, timeout=120, env=env, stdin=subprocess.DEVNULL
        )
        snippet = (p.stdout + p.stderr)[:900]
        out.append(f"**{label}** exit={p.returncode}\n```\n{snippet}\n```")
    return "\n".join(out)


def pick_rows(tsv: Path, hand: str, kind: str | None, n: int) -> list[list[str]]:
    rows: list[list[str]] = []
    for line in tsv.read_text().splitlines()[1:]:
        parts = line.split("\t")
        if len(parts) < 6 or parts[5] != hand:
            continue
        if kind and parts[1] != kind:
            continue
        rows.append(parts)
        if len(rows) >= n:
            break
    return rows


def minimal_diff(original: Path, mutant: Path) -> str:
    if not original.exists() or not mutant.exists():
        return "_mutant or original missing_"
    o = original.read_text().splitlines()
    m = mutant.read_text().splitlines()
    out: list[str] = []
    for i, (a, b) in enumerate(zip(o, m), 1):
        if a != b:
            out.append(f"@@ line {i} @@")
            out.append(f"- {a}")
            out.append(f"+ {b}")
    if len(o) != len(m):
        out.append(f"@@ length {len(o)} -> {len(m)} @@")
    return "\n".join(out[:24]) or "_no line diff (identical)_"


def fixture_reconcile(tsv: Path, shard: int) -> list[str]:
    from collections import Counter

    import subprocess

    proc = subprocess.run(
        [
            "git",
            "-C",
            str(REPO),
            "ls-files",
            "tests/omega/pass/**/main.omg",
            "tests/omega/run/**/main.omg",
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    paths = sorted(proc.stdout.splitlines())
    shard_fixtures = [paths[i] for i in range(len(paths)) if i % 8 == shard]
    rows_per: Counter[str] = Counter()
    no_mut: set[str] = set()
    mutated: set[str] = set()
    mutant_rows = 0
    for line in tsv.read_text().splitlines()[1:]:
        parts = line.split("\t")
        if not parts:
            continue
        fixture, kind = parts[0], parts[1]
        rows_per[fixture] += 1
        if kind == "_no_mutations":
            no_mut.add(fixture)
        elif kind == "_original_not_accepted":
            pass
        elif not kind.startswith("_"):
            mutated.add(fixture)
            mutant_rows += 1
    multi = [f for f, n in rows_per.items() if n > 1]
    return [
        "## Fixture reconciliation",
        "",
        f"| Metric | Count |",
        f"| --- | ---: |",
        f"| Fixtures in shard (index % 8 == {shard}) | {len(shard_fixtures)} |",
        f"| Fixtures with any TSV row | {len(rows_per)} |",
        f"| No-mutation fixtures (`_no_mutations`) | {len(no_mut)} |",
        f"| Mutated fixtures (≥1 mutant row) | {len(mutated)} |",
        f"| Mutant rows (excl. metadata) | {mutant_rows} |",
        f"| Fixtures with >1 TSV row | {len(multi)} |",
        "",
    ] + (
        ["Sample fixtures with multiple rows:", ""]
        + [f"- `{f}` ({rows_per[f]} rows)" for f in sorted(multi)[:15]]
        + (["", f"_…and {len(multi) - 15} more._", ""] if len(multi) > 15 else [""])
        if multi
        else ["", "_No fixture has more than one TSV row._", ""]
    )


def write_section(title: str, rows: list[list[str]], *, with_diff: bool = False) -> list[str]:
    if not rows:
        return [f"## {title}", "", "_No rows in this shard._", ""]
    lines = [f"## {title}", ""]
    for parts in rows:
        rel, kind, mrel = parts[0], parts[1], parts[2]
        lines += [
            f"### {rel} — {kind}",
            "",
            f"- current=`{parts[3]}` route=`{parts[4]}` verdict=`{parts[5]}`",
            f"- obligation: `{parts[8] if len(parts)>8 else ''}`",
            f"- route_reason: `{parts[9] if len(parts)>9 else ''}`",
            f"- review_reason: `{parts[10] if len(parts)>10 else ''}`",
            f"- hole_subtype: `{parts[12] if len(parts)>12 else ''}`",
            "",
        ]
        mutant = REPO / mrel if mrel else None
        original = REPO / rel
        if with_diff and mutant and mutant.exists():
            lines += ["**Minimal diff**", "```diff", minimal_diff(original, mutant), "```", ""]
        if mutant and mutant.exists():
            lines.append(capture(mutant))
            lines.append("")
    return lines


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--shard", type=int, required=True)
    args = ap.parse_args()
    tsv = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{args.shard}.tsv"
    md = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{args.shard}.md"
    totals = _totals_from_tsv(tsv)
    totals["baseline_mutants"] = totals["mutants_run"]
    body = _md_lines(args.shard, Counters(), _holes_from_tsv(tsv), totals)
    body += fixture_reconcile(tsv, args.shard)
    body += write_section(
        "Hand verification: confirmed-hole (5)",
        pick_rows(tsv, "confirmed-hole", None, 5),
        with_diff=True,
    )
    body += write_section(
        "Hand verification: known-gap-q7 remove_mut (5)",
        pick_rows(tsv, "known-gap-q7-mut", "remove_mut", 5),
    )
    body += write_section(
        "Hand verification: discarded drop_requires (5)",
        pick_rows(tsv, "discarded-valid", "drop_requires", 5),
    )
    md.write_text("\n".join(body), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
