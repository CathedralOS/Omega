#!/usr/bin/env python3
"""Post-process shard TSV into fix3 buckets and shard-S.md."""
from __future__ import annotations

import csv
import json
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from shard2_review import (
    accepted_by,
    is_known_q7_gap,
    read_mutant_validity,
    removed_requires_clause,
    requires_repeated_elsewhere,
    run_checker_outputs,
)

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 2
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT / f"shard-{SHARD}.tsv"
MD = OUT / f"shard-{SHARD}.md"
PROGRESS = OUT / f"shard-{SHARD}.progress.json"

FINAL_BUCKETS = (
    "confirmed_hole",
    "known_gap_q7_mut",
    "disagreement_valid",
    "discarded_valid",
    "rejected_both",
    "timeout",
    "failed_run",
)


def checker_accepts(cv: str, rv: str) -> bool:
    return cv == "accepted" or rv == "accepted"


def both_reject(cv: str, rv: str) -> bool:
    if checker_accepts(cv, rv):
        return False
    return cv in ("rejected", "stopped", "timeout", "error") and rv in (
        "rejected",
        "incomplete",
        "defect",
        "outside",
        "fallback",
        "no-entry",
        "unreached",
        "timeout",
        "error",
        "unknown",
    )


def classify_row(row: dict) -> dict:
    cv = row.get("current_verdict", "")
    rv = row.get("route_verdict", "")
    kind = row.get("mutation_kind", "")
    run_disp = row.get("run_disposition", "")
    base = {
        **row,
        "review_bucket": "",
        "accepted_by": accepted_by(cv, rv),
        "validity_reason": "",
        "obligation_line": "",
        "rule_path": "",
        "rule_quote": "",
        "notes": row.get("route_rejection_detail", ""),
    }

    if run_disp == "run_failed":
        base["review_bucket"] = "failed_run"
        return base
    if run_disp == "timeout" or cv == "timeout" or rv == "timeout":
        base["review_bucket"] = "timeout"
        return base
    if both_reject(cv, rv):
        base["review_bucket"] = "rejected_both"
        return base

    mutant_path = (row.get("mutant_path") or "").strip()
    if not mutant_path:
        base["review_bucket"] = "rejected_both"
        base["validity_reason"] = "mutant not retained (neither checker accepted)"
        return base

    orig_path = REPO / row["fixture"]
    mut_path = REPO / mutant_path
    orig = orig_path.read_text()
    mut = mut_path.read_text()
    invalid, reason, oblig, rpath, rquote = read_mutant_validity(
        orig, mut, kind, orig_path
    )
    base["validity_reason"] = reason
    base["obligation_line"] = oblig
    base["rule_path"] = rpath
    base["rule_quote"] = rquote

    ab = accepted_by(cv, rv)

    if kind == "remove_mut" and invalid and oblig:
        bn = binding_from_remove(orig)
        if bn:
            q7, q7s = is_known_q7_gap(orig_path, bn, oblig)
            if q7 and checker_accepts(cv, rv):
                base["review_bucket"] = "known_gap_q7_mut"
                base["notes"] = f"Q7 hit: {q7s}; route hint: {base['notes']}"
                base["rule_path"], _, base["rule_quote"] = (
                    "wiki/spec/language/ownership.md",
                    "",
                    "modifying an established ordinary local's owned storage requires a `mut` binding",
                )
                return base

    if kind in ("ref_to_mut_ref", "mut_ref_to_ref") and invalid and oblig:
        bn = first_mut_binding(orig)
        if bn:
            q7, q7s = is_known_q7_gap(orig_path, bn, oblig)
            if q7 and "mut" in reason.lower() and checker_accepts(cv, rv):
                base["review_bucket"] = "known_gap_q7_mut"
                base["notes"] = f"only missing `mut` on `{bn}`: {q7s}"
                return base

    if invalid and checker_accepts(cv, rv):
        base["review_bucket"] = "confirmed_hole"
        base["accepted_by"] = ab
        return base

    if not invalid and checker_accepts(cv, rv):
        base["review_bucket"] = "discarded_valid"
        return base

    if not invalid and cv != rv:
        base["review_bucket"] = "disagreement_valid"
        base["notes"] = f"read valid; current={cv} route={rv}"
        return base

    if invalid and not checker_accepts(cv, rv):
        base["review_bucket"] = "rejected_both"
        return base

    base["review_bucket"] = "discarded_valid"
    return base


def binding_from_remove(orig: str) -> str | None:
    import re

    m = re.search(r"\blet mut\s+([A-Za-z_][A-Za-z0-9_]*)", orig)
    return m.group(1) if m else None


def remove_mut_name(orig: str) -> str | None:
    return binding_from_remove(orig)


def first_mut_binding(orig: str) -> str | None:
    return binding_from_remove(orig)


def write_tsv(rows: list[dict]) -> None:
    cols = [
        "fixture",
        "tier",
        "mutation_kind",
        "mutant_path",
        "current_verdict",
        "route_verdict",
        "run_disposition",
        "route_rejection_detail",
        "review_bucket",
        "accepted_by",
        "validity_reason",
        "obligation_line",
        "rule_path",
        "rule_quote",
        "notes",
        "seconds",
    ]
    with TSV.open("w") as f:
        f.write("\t".join(cols) + "\n")
        for r in rows:
            f.write("\t".join(str(r.get(c, "")) for c in cols) + "\n")


def hand_block(title: str, rows: list[dict], n: int = 5) -> list[str]:
    if not rows:
        return [f"## {title}", "", "_None._", ""]
    out = [f"## {title}", ""]
    for i, row in enumerate(rows[:n], 1):
        mp = row.get("mutant_path", "")
        lines = [f"### {i}. `{row['fixture']}` ({row['mutation_kind']})", ""]
        if mp:
            orig = (REPO / row["fixture"]).read_text()
            mut = (REPO / mp).read_text()
            ch = run_checker_outputs(REPO / mp)
            lines.extend(
                [
                    f"- Bucket: `{row['review_bucket']}`; accepted_by: `{row.get('accepted_by')}`",
                    f"- Validity (mutant read): {row.get('validity_reason', '')}",
                    f"- Obligation: `{row.get('obligation_line', '')}`",
                    f"- Route hint only: `{row.get('route_rejection_detail', row.get('notes', ''))[:200]}`",
                    f"- omega exit {ch['omega_exit']}; route exit {ch['route_exit']}",
                    "",
                ]
            )
            if row["mutation_kind"] == "drop_machine_requires":
                removed = removed_requires_clause(orig, mut) or "?"
                dup = requires_repeated_elsewhere(orig, removed)
                lines.append(f"- Removed requires: `{removed}`; restated elsewhere: `{dup or 'no'}`")
                lines.append("")
        else:
            lines.append(f"- {row.get('validity_reason', '')}")
            lines.append("")
        out.extend(lines)
    return out


def main() -> None:
    raw = list(csv.DictReader(TSV.open(), delimiter="\t"))
    reviewed = [classify_row(r) for r in raw]
    write_tsv(reviewed)

    bucket_counts = Counter(r["review_bucket"] for r in reviewed)
    total = len(reviewed)
    assert sum(bucket_counts.values()) == total

    confirmed = [r for r in reviewed if r["review_bucket"] == "confirmed_hole"]
    by_kind = Counter(r["mutation_kind"] for r in confirmed)
    by_acc = Counter(r.get("accepted_by", "") for r in confirmed)

    lines = [
        f"# Soundness mutations audit (shard {SHARD} of 8)",
        "",
        f"PIN: `{PIN}`",
        "",
        "## Bucket table (sums to mutant total)",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    for b in FINAL_BUCKETS:
        lines.append(f"| {b} | {bucket_counts.get(b, 0)} |")
    lines.append(f"| **Total mutants** | **{total}** |")

    if PROGRESS.is_file():
        prog = json.loads(PROGRESS.read_text())
        st = prog.get("stats", {})
        fx_done = len(prog.get("fixtures_done", []))
        fx_total = st.get("fixtures_total", "?")
        skipped = st.get("mutator_skipped_no_apply", 0)
        lines.extend(
            [
                "",
                f"- Fixtures covered: {fx_done} / {fx_total}",
                f"- Mutator sites skipped (no-mutation, not in table): {skipped}",
                f"- Mutants per fixture (mean): {total / fx_done:.2f}" if fx_done else "",
            ]
        )

    lines.extend(["", "## Confirmed holes by kind", ""])
    for k, c in sorted(by_kind.items()):
        lines.append(f"- `{k}`: {c}")
    lines.extend(["", "## Confirmed holes by accepted_by", ""])
    for k, c in sorted(by_acc.items()):
        lines.append(f"- `{k}`: {c}")

    lines.extend(hand_block("Hand check: 5 confirmed holes", confirmed))
    lines.extend(
        hand_block(
            "Hand check: 5 discarded-valid",
            [r for r in reviewed if r["review_bucket"] == "discarded_valid"],
        )
    )
    lines.extend(
        hand_block(
            "Hand check: 5 known-gap-q7-mut",
            [r for r in reviewed if r["review_bucket"] == "known_gap_q7_mut"],
        )
    )

    lines.extend(
        [
            "",
            "## Commands",
            "",
            "```bash",
            "PYTHONUNBUFFERED=1 python3 .agents/four-jobs-2026-11-12/soundness/shard2_mutate_run.py",
            "python3 .agents/four-jobs-2026-11-12/soundness/shard2_confirm.py",
            "```",
            "",
        ]
    )

    MD.write_text("\n".join(lines))
    print(bucket_counts)
    print("confirmed", len(confirmed))


if __name__ == "__main__":
    main()
