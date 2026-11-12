#!/usr/bin/env python3
"""Classify every mutant (soundness_fix + soundness_fix2); rewrite TSV + markdown."""

from __future__ import annotations

import subprocess
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path

_SCRIPT_DIR = Path(__file__).resolve().parent
if str(_SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(_SCRIPT_DIR))

from mutation_validate import classify_row, current_accepts
from route_reason_backfill import fetch_route_reason

REPO = Path(__file__).resolve().parents[3]
SHARD = int(__import__("os").environ.get("FOUR_JOBS_SHARD", "7"))
OUT_ROOT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV_IN = OUT_ROOT / f"shard-{SHARD}.tsv"
TSV_OUT = OUT_ROOT / f"shard-{SHARD}.tsv"
MD = OUT_ROOT / f"shard-{SHARD}.md"
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


_BUCKETS = frozenset(
    {
        "confirmed-hole",
        "discarded-valid",
        "known-gap-q7-mut",
        "rejected-both",
        "checker-disagreement",
        "run-error",
        "run-timeout",
        "discarded-no-rule",
        "error-missing-file",
    }
)


def normalize_row(parts: list[str]) -> Row:
    """Accept legacy rows and re-reviewed rows."""
    if len(parts) >= 9 and parts[8] in _BUCKETS:
        return Row(parts[0], parts[1], parts[2], parts[3], parts[4], parts[5], "", parts[6], parts[7])
    if len(parts) >= 10 and parts[6] not in _BUCKETS:
        try:
            float(parts[7])
            # route_reason present at 6
            return Row(parts[0], parts[1], parts[2], parts[3], parts[4], parts[5], parts[6], parts[7], parts[8])
        except ValueError:
            pass
    if len(parts) >= 8:
        return Row(parts[0], parts[1], parts[2], parts[3], parts[4], parts[5], "", parts[6], parts[7])
    raise ValueError(f"bad row: {parts[:5]}")


def read_rows() -> list[Row]:
    rows: list[Row] = []
    for line in TSV_IN.read_text().splitlines():
        if not line or line.startswith("#") or line.startswith("original_fixture"):
            continue
        rows.append(normalize_row(line.split("\t")))
    return rows


def diff_snippet(orig_path: Path, mutant_path: Path, limit: int = 12) -> list[str]:
    o_lines = orig_path.read_text().splitlines()
    m_lines = mutant_path.read_text().splitlines()
    out: list[str] = []
    for i, (a, b) in enumerate(zip(o_lines, m_lines)):
        if a != b:
            out.append(f"- {a}")
            out.append(f"+ {b}")
            if len(out) >= limit:
                break
    return out


def hand_verify(mutant: Path) -> list[str]:
    lines: list[str] = []
    if not OMEGA.is_file() or not ROUTE.is_file():
        return ["(binaries missing)"]
    try:
        omg = subprocess.run(
            [str(OMEGA), "--check", str(mutant)],
            cwd=REPO,
            capture_output=True,
            text=True,
            timeout=120,
            stdin=subprocess.DEVNULL,
        )
        lines.append(f"omega exit={omg.returncode}")
        lines.append(f"omega stdout head: {(omg.stdout or '')[:350].replace(chr(10), ' | ')}")
    except subprocess.TimeoutExpired:
        lines.append("omega: timeout")
    try:
        rt = subprocess.run(
            [str(ROUTE), str(mutant)],
            cwd=REPO,
            capture_output=True,
            text=True,
            timeout=120,
            stdin=subprocess.DEVNULL,
        )
        for ln in (rt.stdout or "").splitlines()[:4]:
            lines.append(ln[:500])
    except subprocess.TimeoutExpired:
        lines.append("route: timeout")
    return lines


def main() -> None:
    if not TSV_IN.exists():
        MD.write_text(f"# Soundness mutations shard {SHARD}\n\n(no data yet)\n")
        return

    raw = read_rows()
    buckets: Counter[str] = Counter()
    classified: list[tuple[Row, str, str, str]] = []
    by_kind_confirmed: Counter[str] = Counter()

    out_lines = [
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
                "obligation_line",
                "review_reason",
            ]
        ),
    ]

    backfill_cache: dict[str, str] = {}

    for row in raw:
        orig_path = REPO / row.orig
        mutant_path = REPO / row.mutant_rel
        route_reason = row.route_reason
        if (
            row.kind == "drop_requires"
            and current_accepts(row.omg, row.cur)
            and not route_reason
        ):
            if row.mutant_rel not in backfill_cache:
                backfill_cache[row.mutant_rel] = fetch_route_reason(mutant_path)
            route_reason = backfill_cache[row.mutant_rel]

        if row.route in ("timeout",) or row.omg == "timeout":
            bucket, obl, reason = "run-timeout", "", "checker timeout"
        elif row.route in ("error",) or row.omg == "error":
            bucket, obl, reason = "run-error", "", "checker error"
        elif not orig_path.is_file() or not mutant_path.is_file():
            bucket, obl, reason = "run-error", "", "missing mutant file"
        else:
            v = classify_row(
                row.kind,
                orig_path,
                mutant_path,
                row.omg,
                row.cur,
                row.route,
                route_reason,
            )
            bucket, obl, reason = v.bucket, v.obligation_line or "", v.notes
            if bucket == "confirmed-hole":
                kind_key = "remove_mut" if row.kind in ("drop_mut", "remove_mut") else row.kind
                by_kind_confirmed[kind_key] += 1

        buckets[bucket] += 1
        classified.append((row, bucket, obl, reason))

        out_lines.append(
            "\t".join(
                [
                    row.orig,
                    row.kind,
                    row.mutant_rel,
                    row.omg,
                    row.cur,
                    row.route,
                    route_reason.replace("\t", " "),
                    row.secs,
                    row.cand,
                    bucket,
                    (obl or "").replace("\t", " "),
                    reason.replace("\t", " "),
                ]
            )
        )

    total = len(raw)
    assert sum(buckets.values()) == total

    generated = run = 0
    if COUNTS.exists():
        for ln in COUNTS.read_text().splitlines():
            if ln.startswith("generated="):
                generated = int(ln.split("=", 1)[1])
            if ln.startswith("run="):
                run = int(ln.split("=", 1)[1])

    confirmed = [x for x in classified if x[1] == "confirmed-hole"]
    q7 = [x for x in classified if x[1] == "known-gap-q7-mut"]
    discarded_dr = [x for x in classified if x[0].kind == "drop_requires" and x[1] == "discarded-valid"]

    md: list[str] = [
        f"# Soundness mutations shard {SHARD} (job 2)",
        "",
        f"PIN: `{PIN}`",
        "",
        "Lead rules: `soundness_fix` + `soundness_fix2` (bucket table sums to mutant",
        "total; drop_requires route reason recorded; Q7 remove_mut shadowing checked).",
        "",
        "## Mutant total",
        "",
        f"- Rows in TSV (mutants run): **{total}**",
        f"- Runner `generated=` counter: **{generated or 'n/a'}**",
        f"- Runner `run=` counter: **{run or 'n/a'}**",
        "",
    ]
    if generated and generated != total:
        md.append(
            f"- Delta: shard still running or partial resume ({total} logged vs {generated} generated)."
        )
    else:
        md.append("- Delta: none (TSV row count matches completed runner total).")
    md.append("")

    md.extend(["## Bucket table", "", f"Total: **{total}**", "", "| Bucket | Count |", "| --- | ---: |"])
    for name in sorted(buckets.keys()):
        md.append(f"| `{name}` | {buckets[name]} |")
    md.append(f"| **sum** | **{sum(buckets.values())}** |")
    md.append("")
    md.append("### Confirmed holes by mutation kind")
    md.append("")
    for k, n in by_kind_confirmed.most_common():
        md.append(f"- `{k}`: {n}")
    if not by_kind_confirmed:
        md.append("(none)")
    md.append("")

    md.extend(["## Confirmed holes (summary)", ""])
    for row, bucket, obl, reason in confirmed[:40]:
        md.append(
            f"- `{row.kind}` `{row.orig}` — omega **{row.omg}**, current **{row.cur}**, "
            f"route **{row.route}**; obligation `{obl}`; {reason[:120]}"
        )
    md.append("")

    md.extend(["## Hand verification: 5 confirmed holes", ""])
    for row, _, obl, reason in confirmed[:5]:
        mutant_path = REPO / row.mutant_rel
        md.extend([f"### {row.kind}", f"- `{row.orig}` → `{row.mutant_rel}`", f"- {reason}", ""])
        md.extend(["```diff"] + diff_snippet(REPO / row.orig, mutant_path) + ["```", ""])
        for ln in hand_verify(mutant_path):
            md.append(f"- {ln}")
        md.append("")

    md.extend(["## Hand verification: 5 discarded drop_requires", ""])
    shown = 0
    for row, _, obl, reason in discarded_dr:
        if not current_accepts(row.omg, row.cur):
            continue
        if shown >= 5:
            break
        shown += 1
        mutant_path = REPO / row.mutant_rel
        rreason = backfill_cache.get(row.mutant_rel, row.route_reason)
        md.extend(
            [
                f"### discarded drop_requires",
                f"- `{row.orig}`",
                f"- route **{row.route}**; reason: {rreason or reason}",
                "",
            ]
        )
        md.extend(["```diff"] + diff_snippet(REPO / row.orig, mutant_path, 8) + ["```", ""])
        for ln in hand_verify(mutant_path):
            md.append(f"- {ln}")
        md.append("")

    md.extend(["## Hand verification: 5 known-gap Q7 remove_mut", ""])
    q7_rows = [x for x in q7 if x[0].kind in ("drop_mut", "remove_mut")][:5]
    for row, _, obl, reason in q7_rows:
        mutant_path = REPO / row.mutant_rel
        md.extend([f"### remove_mut (Q7)", f"- obligation: `{obl}`", f"- `{row.orig}`", ""])
        md.extend(["```diff"] + diff_snippet(REPO / row.orig, mutant_path, 6) + ["```", ""])
        for ln in hand_verify(mutant_path):
            md.append(f"- {ln}")
        md.append("")

    TSV_OUT.write_text("\n".join(out_lines) + "\n")
    MD.write_text("\n".join(md) + "\n")
    print(
        f"classified {total}; confirmed={len(confirmed)} q7={len(q7)} "
        f"discarded_drop_requires={len(discarded_dr)}"
    )


if __name__ == "__main__":
    main()
