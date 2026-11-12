#!/usr/bin/env python3
"""Classify every mutant per lead soundness_fix rules; rewrite TSV + shard markdown."""

from __future__ import annotations

import subprocess
import sys
from collections import Counter
from pathlib import Path

_SCRIPT_DIR = Path(__file__).resolve().parent
if str(_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPT_DIR))

from mutation_validate import classify_row, current_accepts

REPO = Path(__file__).resolve().parents[3]
SHARD = int(__import__("os").environ.get("FOUR_JOBS_SHARD", "7"))
OUT_ROOT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV_IN = OUT_ROOT / f"shard-{SHARD}.tsv"
TSV_OUT = OUT_ROOT / f"shard-{SHARD}.tsv"
MD = OUT_ROOT / f"shard-{SHARD}.md"
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"

OMEGA = REPO / "target/debug/omega"
ROUTE = REPO / "target/debug/examples/route_one_comparison"


def read_rows() -> list[list[str]]:
    rows: list[list[str]] = []
    for line in TSV_IN.read_text().splitlines():
        if not line or line.startswith("#") or line.startswith("original_fixture"):
            continue
        parts = line.split("\t")
        while len(parts) < 9:
            parts.append("")
        rows.append(parts[:9])
    return rows


def hand_verify(mutant: Path) -> list[str]:
    lines: list[str] = []
    if not OMEGA.is_file() or not ROUTE.is_file():
        lines.append("(binaries missing — hand-verify skipped)")
        return lines
    main = mutant if mutant.name == "main.omg" else mutant
    try:
        omg = subprocess.run(
            [str(OMEGA), "--check", str(main)],
            cwd=REPO,
            capture_output=True,
            text=True,
            timeout=120,
            stdin=subprocess.DEVNULL,
        )
        lines.append(f"omega exit={omg.returncode}")
        out = (omg.stdout or "")[:400].replace("\n", " | ")
        err = (omg.stderr or "")[:200].replace("\n", " | ")
        lines.append(f"omega stdout: {out}")
        if err:
            lines.append(f"omega stderr: {err}")
    except subprocess.TimeoutExpired:
        lines.append("omega: timeout")
    try:
        rt = subprocess.run(
            [str(ROUTE), str(main)],
            cwd=REPO,
            capture_output=True,
            text=True,
            timeout=120,
            stdin=subprocess.DEVNULL,
        )
        prog = [ln for ln in (rt.stdout or "").splitlines() if ln.startswith("program\t")][:3]
        lines.append(f"route exit={rt.returncode}")
        for ln in prog:
            lines.append(f"route: {ln[:500]}")
    except subprocess.TimeoutExpired:
        lines.append("route: timeout")
    return lines


def main() -> None:
    if not TSV_IN.exists():
        MD.write_text(f"# Soundness mutations shard {SHARD}\n\n(no data yet)\n")
        return

    raw = read_rows()
    buckets: Counter[str] = Counter()
    classified: list[tuple[list[str], object]] = []
    by_kind_confirmed: Counter[str] = Counter()

    out_lines = [
        f"# pin={PIN} shard={SHARD}",
        "original_fixture\tmutation_kind\tmutant_path\tomega_verdict\troute_current_verdict\troute_verdict\tseconds\tcandidate\treview_bucket\tobligation_line",
    ]

    for parts in raw:
        orig, kind, mutant_rel, omg, cur, route, secs, cand, _old = parts
        orig_path = REPO / orig
        mutant_path = REPO / mutant_rel
        if not orig_path.is_file() or not mutant_path.is_file():
            buckets["error-missing-file"] += 1
            bucket = "error-missing-file"
            obl = ""
            v_notes = ""
        else:
            v = classify_row(kind, orig_path, mutant_path, omg, cur, route)
            bucket = v.bucket
            obl = v.obligation_line or ""
            v_notes = v.notes
            if bucket == "confirmed-hole":
                kind_key = "remove_mut" if kind in ("drop_mut", "remove_mut") else kind
                by_kind_confirmed[kind_key] += 1
        buckets[bucket] += 1
        classified.append((parts, (bucket, obl, v_notes)))

        out_lines.append(
            "\t".join(
                [
                    orig,
                    kind,
                    mutant_rel,
                    omg,
                    cur,
                    route,
                    secs,
                    cand,
                    bucket,
                    obl.replace("\t", " "),
                ]
            )
        )

    total = len(raw)
    assert sum(buckets.values()) == total, f"buckets {sum(buckets.values())} != {total}"

    confirmed = [x for x in classified if x[1][0] == "confirmed-hole"]
    q7 = [x for x in classified if x[1][0] == "known-gap-q7-mut"]
    disagree = [x for x in classified if x[1][0] == "checker-disagreement"]

    md: list[str] = [
        f"# Soundness mutations shard {SHARD} (job 2)",
        "",
        f"PIN: `{PIN}`",
        "",
        "Classification follows lead `soundness_fix` (2026-11-12): bucket table sums to",
        "mutant total; `drop_requires` needs a body obligation; `remove_mut` → Q7 bucket;",
        "confirmed holes require current-checker accept + spec-invalid mutant.",
        "",
        "## Commands",
        "",
        "```bash",
        "cd /agent/repos/omega",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --example route_one_comparison",
        f"FOUR_JOBS_SHARD={SHARD} python3 .agents/four-jobs-2026-11-12/soundness/run_shard_mutations.py",
        f"FOUR_JOBS_SHARD={SHARD} python3 .agents/four-jobs-2026-11-12/soundness/review_shard_candidates.py",
        "```",
        "",
        "## Bucket table (sums to mutant total)",
        "",
        f"Total mutants run: **{total}**",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    for name in sorted(buckets.keys()):
        md.append(f"| `{name}` | {buckets[name]} |")
    md.append(f"| **sum** | **{sum(buckets.values())}** |")
    md.append("")
    md.append(
        "Confirmed hole = spec-invalid mutant **and** current checker accepts "
        "(omega or route `current=`). Route accept alone is not evidence."
    )
    md.append("")
    md.append("### Confirmed holes by mutation kind")
    md.append("")
    if by_kind_confirmed:
        for k, n in by_kind_confirmed.most_common():
            md.append(f"- `{k}`: {n}")
    else:
        md.append("(none)")
    md.append("")

    md.extend(["## Known gap: Q7 mut", ""])
    if not q7:
        md.append(f"({len(q7)} rows; see TSV `known-gap-q7-mut`.)")
    else:
        md.append(f"{len(q7)} rows bucketed as tracked missing-`mut` enforcement (not spec holes).")
    md.append("")

    md.extend(["## Confirmed holes", ""])
    for parts, (bucket, obl, _notes) in confirmed[:50]:
        orig, kind, mutant_rel, omg, cur, route, *_ = parts
        mutant_path = REPO / mutant_rel
        v = classify_row(kind, REPO / orig, mutant_path, omg, cur, route)
        spec = v.spec_path or ""
        quote = v.spec_quote or ""
        heading = v.spec_heading or ""
        md.extend(
            [
                f"### {kind} — `{orig}`",
                "",
                f"- Mutant: `{mutant_rel}`",
                f"- omega: **{omg}**; current: **{cur}**; route: **{route}**",
                f"- Obligation line: `{obl}`",
                f"- Spec: `wiki/spec/{spec}` ({heading})",
                "",
                f"> {quote}",
                "",
            ]
        )

    md.extend(["## Checker disagreements", ""])
    for parts, _ in disagree[:30]:
        orig, kind, mutant_rel, omg, cur, route, *_ = parts
        md.append(
            f"- `{kind}` `{orig}` — omega **{omg}**, current **{cur}**, route **{route}** (`{mutant_rel}`)"
        )
    md.append("")

    md.extend(["## Hand verification (5 samples)", ""])
    samples = confirmed[:5]
    if not samples:
        samples = q7[:5]
    for parts, (bucket, obl, notes) in samples:
        orig, kind, mutant_rel, omg, cur, route, *_ = parts
        mutant_path = REPO / mutant_rel
        orig_path = REPO / orig
        md.extend([f"### Sample: {kind} / {bucket}", "", f"- Original: `{orig}`", f"- Mutant: `{mutant_rel}`", ""])
        md.append("**Diff (first `let mut` change or removed `requires`):**")
        md.append("```omg")
        o_lines = orig_path.read_text().splitlines()
        m_lines = mutant_path.read_text().splitlines()
        for i, (a, b) in enumerate(zip(o_lines, m_lines)):
            if a != b:
                md.append(f"- {a}")
                md.append(f"+ {b}")
                if i > 8:
                    break
        if len(o_lines) != len(m_lines):
            md.append(f"({len(o_lines)} vs {len(m_lines)} lines)")
        md.append("```")
        md.append("")
        for ln in hand_verify(mutant_path):
            md.append(f"- {ln}")
        md.append("")

    TSV_OUT.write_text("\n".join(out_lines) + "\n")
    MD.write_text("\n".join(md) + "\n")
    print(f"classified {total} mutants; confirmed-hole={len(confirmed)} q7={len(q7)} disagree={len(disagree)}")


if __name__ == "__main__":
    main()
