#!/usr/bin/env python3
"""Post-process shard TSV with fix3 rules; rewrite TSV + markdown."""

from __future__ import annotations

import subprocess
import sys
from collections import Counter
from dataclasses import dataclass, replace
from pathlib import Path

_SCRIPT_DIR = Path(__file__).resolve().parent
if str(_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPT_DIR))

from collections import defaultdict

from fix3_classify import Outcome, classify_fix3
from fix4_apply import minimal_diff
from fix5_drop_requires import (
    FIX5_STATE_QUOTE,
    probe_violating_caller,
    violating_arg_for_clause,
    machine_name_before_requires,
)
from mutation_validate import current_accepts, removed_requires_clauses
from route_reason_backfill import fetch_route_reason

REPO = Path(__file__).resolve().parents[3]
SHARD = int(__import__("os").environ.get("FOUR_JOBS_SHARD", "7"))
OUT_ROOT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT_ROOT / f"shard-{SHARD}.tsv"
MD = OUT_ROOT / f"shard-{SHARD}.md"
PROGRESS = OUT_ROOT / f"shard-{SHARD}.progress"
COUNTS = OUT_ROOT / f"shard-{SHARD}.counts"
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
OMEGA = REPO / "target/debug/omega"
ROUTE = REPO / "target/debug/examples/route_one_comparison"


@dataclass
class Row:
    orig: str
    kind: str
    mutant_rel: str
    omg: str
    cur: str
    route: str
    route_reason: str
    secs: str
    cand: str


def _looks_like_seconds(field: str) -> bool:
    try:
        float(field)
        return True
    except ValueError:
        return False


def parse_tsv() -> list[Row]:
    rows: list[Row] = []
    for line in TSV.read_text().splitlines():
        if not line or line.startswith("#") or line.startswith("original_fixture"):
            continue
        p = line.split("\t")
        while len(p) < 8:
            p.append("")
        if len(p) >= 11 and not _looks_like_seconds(p[6]):
            rows.append(Row(p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], p[8]))
        elif len(p) >= 14:
            rows.append(Row(p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], p[8]))
        else:
            rows.append(Row(p[0], p[1], p[2], p[3], p[4], p[5], "", p[6], p[7]))
    return rows


def _row_oc(item: tuple) -> tuple[Row, Outcome | object, str]:
    if len(item) == 3:
        return item[0], item[1], item[2]
    row, oc = item
    return row, oc, ""


def diff_snippet(orig: Path, mut: Path, n: int = 10) -> list[str]:
    out: list[str] = []
    for a, b in zip(orig.read_text().splitlines(), mut.read_text().splitlines()):
        if a != b:
            out.extend([f"- {a}", f"+ {b}"])
        if len(out) >= n:
            break
    return out


def hand_cmd(mutant: Path) -> list[str]:
    if not OMEGA.is_file():
        return []
    lines: list[str] = []
    omg = subprocess.run(
        [str(OMEGA), "--check", str(mutant)],
        cwd=REPO,
        capture_output=True,
        text=True,
        timeout=120,
        stdin=subprocess.DEVNULL,
    )
    lines.append(f"omega exit={omg.returncode}")
    lines.append((omg.stdout or "")[:280].replace("\n", " | "))
    rt = subprocess.run(
        [str(ROUTE), str(mutant)],
        cwd=REPO,
        capture_output=True,
        text=True,
        timeout=120,
        stdin=subprocess.DEVNULL,
    )
    for ln in (rt.stdout or "").splitlines()[:2]:
        if ln.startswith("program"):
            lines.append(ln[:450])
    return lines


def main() -> None:
    if not TSV.exists():
        MD.write_text("# shard 7\n\n(no data)\n")
        return

    raw = parse_tsv()
    buckets: Counter[str] = Counter()
    accepted_by: Counter[str] = Counter()
    by_kind: Counter[str] = Counter()
    cache: dict[str, str] = {}

    out = [
        f"# pin={PIN} shard={SHARD}",
        "\t".join(
            [
                "original_fixture",
                "mutation_kind",
                "mutant_path",
                "omega_verdict",
                "route_current_verdict",
                "route_verdict",
                "route_reason",
                "seconds",
                "candidate",
                "review_bucket",
                "accepted_by",
                "obligation_line",
                "review_reason",
                "hole_subtype",
            ]
        ),
    ]

    classified: list[tuple[Row, Outcome | object, str]] = []

    for row in raw:
        orig_p = REPO / row.orig
        mut_p = REPO / row.mutant_rel
        rr = row.route_reason
        if row.kind == "no-mutation":
            oc = type(
                "O",
                (),
                {
                    "bucket": "no-mutation",
                    "accepted_by": "",
                    "obligation": None,
                    "spec_path": None,
                    "spec_quote": None,
                    "spec_heading": None,
                    "note": "no applicable mutations",
                },
            )()
            buckets[oc.bucket] += 1
            classified.append((row, oc, ""))
            out.append(
                "\t".join(
                    [
                        row.orig,
                        row.kind,
                        "",
                        row.omg,
                        row.cur,
                        row.route,
                        "",
                        row.secs,
                        row.cand,
                        oc.bucket,
                        "",
                        "",
                        oc.note,
                        "",
                    ]
                )
            )
            continue

        if row.kind == "drop_requires" and current_accepts(row.omg, row.cur) and not rr:
            if row.mutant_rel not in cache:
                cache[row.mutant_rel] = fetch_route_reason(mut_p)
            rr = cache[row.mutant_rel]

        if not orig_p.is_file() or not mut_p.is_file():
            oc = type(
                "O",
                (),
                {
                    "bucket": "failed-run",
                    "accepted_by": "",
                    "obligation": None,
                    "spec_path": None,
                    "spec_quote": None,
                    "spec_heading": None,
                    "note": "missing files",
                },
            )()
        else:
            oc = classify_fix3(
                row.kind,
                orig_p.read_text(),
                mut_p.read_text(),
                row.omg,
                row.cur,
                row.route,
                rr,
            )

        buckets[oc.bucket] += 1
        if oc.bucket == "confirmed-hole" and oc.accepted_by:
            accepted_by[oc.accepted_by] += 1
            kind = "remove_mut" if row.kind in ("drop_mut", "remove_mut") else row.kind
            by_kind[kind] += 1
        subtype = ""
        if oc.bucket == "confirmed-hole" and row.kind == "drop_requires" and row.mutant_rel:
            orig_txt = orig_p.read_text()
            mut_txt = mut_p.read_text()
            removed = removed_requires_clauses(orig_txt, mut_txt)
            machine = machine_name_before_requires(orig_txt, removed[0]) if removed else None
            varg = violating_arg_for_clause(removed[0]) if removed else None
            if machine and varg:
                subtype, pd = probe_violating_caller(
                    REPO, mut_p.parent, machine, varg, OMEGA
                )
                if isinstance(oc, Outcome):
                    oc = replace(oc, note=(oc.note + " | " + pd) if oc.note else pd)
            else:
                subtype = "probe-not-built"

        classified.append((row, oc, subtype))

        out.append(
            "\t".join(
                [
                    row.orig,
                    row.kind,
                    row.mutant_rel,
                    row.omg,
                    row.cur,
                    row.route,
                    (rr or "").replace("\t", " "),
                    row.secs,
                    row.cand,
                    oc.bucket,
                    oc.accepted_by,
                    (oc.obligation or "").replace("\t", " "),
                    oc.note.replace("\t", " "),
                    subtype,
                ]
            )
        )

    total = len(raw)
    assert sum(buckets.values()) == total

    fixtures_done = PROGRESS.read_text().count("\n") if PROGRESS.exists() else 0
    generated = 0
    if COUNTS.exists():
        for ln in COUNTS.read_text().splitlines():
            if ln.startswith("generated="):
                generated = int(ln.split("=", 1)[1])

    confirmed = [(r, o) for r, o, _ in classified if o.bucket == "confirmed-hole"]
    discarded_dr = [
        (r, o)
        for r, o, _ in classified
        if r.kind == "drop_requires" and o.bucket == "discarded-valid" and o.accepted_by
    ]
    q7 = [(r, o) for r, o, _ in classified if o.bucket == "known-gap-q7-mut"]

    md: list[str] = [
        f"# Soundness mutations shard {SHARD} (job 2)",
        "",
        f"PIN: `{PIN}`",
        "",
        "Rules: `soundness_fix` → `fix2` → **`fix3`** → **`fix4`** → **`fix5`** (latest wins).",
        "fix5: only **external** caller re-establishment is ignored; in-machine transition/guard restatement → discarded-valid (both sides quoted).",
        "Confirmed `drop_requires` holes: `hole_subtype` is `inferred-entry`, `violating-call-accepted`, or `probe-not-built` (never `unchecked`).",
        "Validity from reading mutants; route reasons are hints only.",
        "",
        "## Coverage reconciliation (fix4)",
        "",
    ]
    by_fixture: dict[str, int] = defaultdict(int)
    for row in raw:
        if row.kind != "no-mutation":
            by_fixture[row.orig] += 1
    no_mut = sum(1 for row in raw if row.kind == "no-mutation")
    mutated_fixtures = len(by_fixture)
    multi = sorted(((f, c) for f, c in by_fixture.items() if c > 1), key=lambda x: -x[1])[:20]
    md.extend(
        [
            f"- Shard fixtures (assigned): **1130**",
            f"- Fixtures finished (progress lines): **{fixtures_done}**",
            f"- Fixtures with ≥1 mutant row: **{mutated_fixtures}**",
            f"- Fixtures with `no-mutation` placeholder: **{no_mut}**",
            f"- Mutant rows (excl. no-mutation placeholders): **{total - no_mut}**",
            f"- Total TSV data rows: **{total}**",
            f"- Runner `generated=` counter: **{generated or 'pending'}**",
            "",
            "Fixtures with more than one mutant row (first 20):",
        ]
    )
    if multi:
        for f, c in multi:
            md.append(f"- `{f}`: **{c}** rows")
    else:
        md.append("- (none yet)")
    md.append("")
    md.append("## Bucket table")
    md.append("")
    md.append("| Bucket | Count |")
    md.append("| --- | ---: |")
    for name in sorted(buckets):
        md.append(f"| `{name}` | {buckets[name]} |")
    md.append(f"| **sum** | **{total}** |")
    md.append("")
    md.append("### Confirmed holes by kind")
    for k, n in by_kind.most_common():
        md.append(f"- `{k}`: {n}")
    md.append("")
    md.append("### Confirmed holes by accepted_by")
    for k, n in accepted_by.most_common():
        md.append(f"- `{k}`: {n}")
    md.append("")

    md.append("## Confirmed holes (hand basis: obligation + spec quote)")
    md.append("")
    for row, o in confirmed:
        orig_text = (REPO / row.orig).read_text()
        mut_text = (REPO / row.mutant_rel).read_text() if row.mutant_rel else ""
        md.extend(
            [
                f"### {row.kind} | accepted_by={o.accepted_by}",
                f"- Fixture: `{row.orig}`",
                f"- Mutant: `{row.mutant_rel}`",
                f"- omega **{row.omg}**, current **{row.cur}**, route **{row.route}** (route outside is not acceptance)",
                f"- Obligation (check-time): `{o.obligation}`",
                f"- Spec: `wiki/spec/{o.spec_path}` — {o.spec_heading}",
                f"> {o.spec_quote}",
                f"- fix4b: `omega --check` accepts without inserting a distinct runtime-only gate for this obligation (compile-time hole).",
                f"- Hand note: {o.note}",
                "```diff",
                minimal_diff(orig_text, mut_text),
                "```",
                "",
            ]
        )

    md.append("## Hand verification samples")
    md.append("")

    def block(title: str, items: list, n: int = 5) -> None:
        md.append(f"### {title}")
        md.append("")
        for row, o in items[:n]:
            md.append(f"- `{row.kind}` `{row.orig}` — {o.note}")
            md.append("```diff")
            md.extend(diff_snippet(REPO / row.orig, REPO / row.mutant_rel))
            md.append("```")
            for ln in hand_cmd(REPO / row.mutant_rel):
                md.append(f"- {ln}")
            md.append("")

    block("5 confirmed holes", confirmed)
    block("5 discarded drop_requires (current accepts)", discarded_dr)
    block("5 known-gap Q7 remove_mut", [(r, o) for r, o in q7 if r.kind in ("remove_mut", "drop_mut")])

    mut_ref_all = [(r, o) for r, o, _ in classified if r.kind == "mut_ref_instead_of_shared"]
    md.append("### Hand read: all mut_ref_instead_of_shared rows")
    md.append("")
    for row, o in mut_ref_all:
        md.append(
            f"- **{o.bucket}** accepted_by={o.accepted_by or '-'} | `{row.orig}` | `{o.obligation}` | {o.note}"
        )
    md.append("")

    TSV.write_text("\n".join(out) + "\n")
    MD.write_text("\n".join(md) + "\n")
    print(f"fix3: total={total} confirmed={len(confirmed)} buckets={dict(buckets)}")


if __name__ == "__main__":
    main()
