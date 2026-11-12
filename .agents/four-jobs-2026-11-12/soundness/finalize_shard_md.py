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
from soundness_validate import Q7_RULE  # noqa: E402


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


def explicit_reconcile(tsv: Path, shard: int) -> list[str]:
    from run_shard_mutations import _totals_from_tsv, shard_fixtures  # noqa: WPS433

    totals = _totals_from_tsv(tsv)
    fixtures = len(shard_fixtures(shard))
    no_mut = totals["no_mutation"]
    failed_orig = totals.get("failed_run_original", 0)
    mutated_fixtures = fixtures - no_mut - failed_orig
    # sanity: disjoint fixture ledger
    failed_fix: set[str] = set()
    no_mut_fix: set[str] = set()
    mut_fix: set[str] = set()
    for line in tsv.read_text().splitlines()[1:]:
        p = line.split("\t")
        if len(p) < 2:
            continue
        if p[1] == "_no_mutations":
            (failed_fix if p[5] == "failed-run" else no_mut_fix).add(p[0])
        elif not p[1].startswith("_"):
            mut_fix.add(p[0])
    ledger_ok = len(failed_fix | no_mut_fix | mut_fix) == fixtures
    mutated_fixtures = len(mut_fix - failed_fix - no_mut_fix)
    no_mut = len(no_mut_fix - mut_fix - failed_fix)
    failed_orig = len(failed_fix)
    bucket_sum = (
        totals["confirmed_hole"]
        + totals["known_gap_q7"]
        + totals["disagreement_valid"]
        + totals["valid_route_outside"]
        + totals["discarded_valid"]
        + totals["rejected_both"]
        + totals["timeout"]
        + totals["failed_run"]
    )
    lines = [
        "## Ledger reconciliation",
        "",
        f"| Identity | Count |",
        f"| --- | ---: |",
        f"| Shard fixtures | {fixtures} |",
        f"| = no-mutation + mutated + original failed-run | {no_mut} + {mutated_fixtures} + {failed_orig} = {no_mut + mutated_fixtures + failed_orig} |",
        f"| Mutant rows in TSV | {totals['mutants_run']} |",
        f"| Mutant bucket sum | {bucket_sum} |",
        f"| Buckets match mutant total | {'yes' if bucket_sum == totals['mutants_run'] else 'NO'} |",
        f"| Fixture ledger disjoint (1131) | {'yes' if ledger_ok else 'NO'} |",
        "",
    ]
    hdr = 0
    body = 0
    for line in tsv.read_text().splitlines()[1:]:
        p = line.split("\t")
        if len(p) < 6 or p[5] != "confirmed-hole" or p[1] != "drop_requires":
            continue
        sub = p[12] if len(p) > 12 else ""
        if sub.startswith("header"):
            hdr += 1
        elif sub.startswith("body"):
            body += 1
    lines += [
        f"**drop_requires confirmed-hole placement:** header {hdr}, body {body} (see `hole_subtype` prefix).",
        "",
    ]
    return lines


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
    drop_rows = sum(
        1
        for line in tsv.read_text().splitlines()[1:]
        if line.split("\t")[1:2] == ["drop_requires"]
    )
    body += [
        "## drop_requires coverage note",
        "",
        "Shard 0 initially recorded **zero** `drop_requires` rows through the first ~968 fixtures because "
        "`mut_drop_first_requires` used a broken parenthesis walk on `requires (...)` clauses (it never "
        "closed the opening `(`) and only matched line-start `requires`, not **machine-header** "
        "`machine … requires … {` forms. Other shards (~100 rows each) likely ran an earlier build "
        "before that regression or used different fixture mix; the bug suppressed **all** drop mutants "
        "here until `requires_span.py` fixed parsing and `backfill_drop_requires.py` re-ran the kind "
        f"for fixtures already in `shard-{args.shard}.progress`. After backfill this shard has "
        f"**{drop_rows}** `drop_requires` TSV rows.",
        "",
    ]
    body += [
        "## Classification rules (shard 0)",
        "",
        f"**Q7 vs confirmed-hole:** {Q7_RULE}",
        "",
        "**flip_borrow_mutability:** exclusive→shared with **no write through the shared borrow** in "
        "executable code → `discarded-valid`; otherwise → `known-gap-q7-mut` when accepted.",
        "",
        "**widen_requires:** invalid only when the inverted `!= 0` guard is not backed by an in-body "
        "`match … != 0` and executable division remains in the mutant.",
        "",
        "**drop_requires:** invalid when requires was removed, no internal restatement, and a "
        "check-time divide/index/`ensures` obligation remains; header vs body tracked in `hole_subtype`.",
        "",
    ]
    import subprocess

    audit = subprocess.run(
        [sys.executable, str(REPO / ".agents/four-jobs-2026-11-12/soundness/audit_no_mutation.py"), "--shard", str(args.shard)],
        capture_output=True,
        text=True,
        cwd=str(REPO),
    )
    body += ["## no-mutation breakdown", "", "```", (audit.stdout or audit.stderr).strip(), "```", ""]
    skip_report = REPO / f".agents/four-jobs-2026-11-12/soundness/shard-{args.shard}.original-skip-report.json"
    if skip_report.exists():
        import json

        items = json.loads(skip_report.read_text())
        body += [
            "## Original failed-run fixtures (PIN)",
            "",
            f"These **{len(items)}** fixtures did not accept on the original at PIN; each has a "
            "`_no_mutations` row with `hand_verdict=failed-run` and a `review_reason`.",
            "",
        ]
        for row in items:
            body.append(
                f"- `{row['fixture']}` — **{row.get('current', '?')}** / route **{row.get('route', '?')}**: "
                f"{row.get('reason', '')[:240]}"
            )
        body.append("")
    body += explicit_reconcile(tsv, args.shard)
    body += fixture_reconcile(tsv, args.shard)
    for kind, n in (
        ("drop_requires", 4),
        ("remove_mut", 4),
        ("flip_borrow_mutability", 4),
        ("widen_requires", 4),
        (None, 4),
    ):
        title = f"Hand verification: confirmed-hole {kind or 'any'} ({n})"
        body += write_section(
            title,
            pick_rows(tsv, "confirmed-hole", kind, n),
            with_diff=True,
        )
    body += write_section(
        "Hand verification: known-gap-q7 remove_mut (4)",
        pick_rows(tsv, "known-gap-q7-mut", "remove_mut", 4),
    )
    body += write_section(
        "Hand verification: discarded drop_requires (4)",
        pick_rows(tsv, "discarded-valid", "drop_requires", 4),
    )
    md.write_text("\n".join(body), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
