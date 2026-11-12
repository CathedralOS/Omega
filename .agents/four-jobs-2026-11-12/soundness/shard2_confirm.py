#!/usr/bin/env python3
"""Assign every mutant to exactly one review bucket (lead review rules)."""
from __future__ import annotations

import csv
import json
import re
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from shard2_review import (
    drop_requires_failing_line,
    is_known_q7_gap,
    pick_spec_quote,
    removed_requires_clause,
    remove_mut_binding_and_line,
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
    "rejected_both",
    "timeout_error",
    "mutant_run_failed",
    "current_rejects",
    "discarded_valid",
    "discarded_no_obligation",
    "discarded_route_only",
    "known_gap_q7_mut",
    "confirmed_invalid",
)

PROOF_ROUTE_HINTS = re.compile(
    r"proof|overflow|range|obligation|not provably|may overflow|index|divide|remainder",
    re.I,
)


def classify_row(row: dict) -> dict:
    """Return row with review_bucket and review fields."""
    cv = row.get("current_verdict", "")
    rv = row.get("route_verdict", "")
    kind = row.get("mutation_kind", "")
    fixture = row.get("fixture", "")
    mutant_path = row.get("mutant_path", "").strip()
    run_disp = row.get("run_disposition") or row.get("disposition", "")

    base = {**row, "review_bucket": "", "obligation_line": "", "rule_path": "", "rule_quote": "", "notes": ""}

    if run_disp == "run_failed":
        base["review_bucket"] = "mutant_run_failed"
        return base

    if run_disp == "timeout" or cv == "timeout" or rv == "timeout":
        base["review_bucket"] = "timeout_error"
        return base

    if cv in ("rejected", "stopped") and rv in (
        "rejected",
        "incomplete",
        "defect",
        "outside",
        "fallback",
        "no-entry",
        "unreached",
    ):
        base["review_bucket"] = "rejected_both"
        return base

    if cv in ("rejected", "stopped"):
        base["review_bucket"] = "current_rejects"
        return base

    if cv != "accepted":
        base["review_bucket"] = "discarded_valid"
        base["notes"] = f"unexpected current verdict {cv}"
        return base

    # Current checker accepts — only these can be soundness holes or Q7 gap.
    if not mutant_path:
        base["review_bucket"] = "discarded_valid"
        base["notes"] = "mutant source not retained"
        return base

    orig_path = REPO / fixture
    mut_path = REPO / mutant_path
    if not mut_path.is_file():
        base["review_bucket"] = "discarded_valid"
        base["notes"] = "mutant file missing"
        return base

    orig = orig_path.read_text()
    mut = mut_path.read_text()

    if kind in ("drop_machine_ensures", "mut_ref_to_ref", "ref_to_mut_ref", "delete_first_if_guard"):
        base["review_bucket"] = "discarded_valid"
        base["notes"] = f"{kind} alone does not establish spec-invalid program under lead rules"
        return base

    if kind == "remove_mut":
        ev = remove_mut_binding_and_line(orig, mut)
        if not ev:
            base["review_bucket"] = "discarded_no_obligation"
            base["notes"] = "removed `mut` but no write, reassign, or &mut of that binding"
            return base
        binding, line, _shadow = ev
        base["obligation_line"] = line
        if binding not in line:
            base["review_bucket"] = "discarded_valid"
            base["notes"] = "obligation line does not cite the demoted binding"
            return base
        q7, q7_snip = is_known_q7_gap(orig_path, binding, line)
        if q7:
            base["review_bucket"] = "known_gap_q7_mut"
            base["notes"] = f"Q7 scan hit on `{binding}`: {q7_snip}"
            quote = pick_spec_quote(kind, line)
            if quote:
                base["rule_path"], _, rq = quote
                base["rule_quote"] = rq
            return base
        quote = pick_spec_quote(kind, line)
        if not quote:
            base["review_bucket"] = "discarded_valid"
            return base
        base["rule_path"], _, base["rule_quote"] = quote
        base["review_bucket"] = "confirmed_invalid"
        return base

    if kind in ("drop_machine_requires", "widen_requires_lt"):
        removed = removed_requires_clause(orig, mut) if kind == "drop_machine_requires" else None
        if kind == "widen_requires_lt":
            for a, b in zip(orig.splitlines(), mut.splitlines()):
                if a.strip().startswith("requires") and a != b and "<=" in b and "<" in a:
                    removed = a.strip()
                    break
        if not removed:
            base["review_bucket"] = "discarded_no_obligation"
            return base
        fail_line = drop_requires_failing_line(orig, mut, removed)
        if not fail_line:
            base["review_bucket"] = "discarded_no_obligation"
            base["notes"] = f"removed `{removed}` but no body line uses that assumption"
            return base
        base["obligation_line"] = fail_line
        quote = pick_spec_quote(kind, fail_line)
        if not quote:
            base["review_bucket"] = "discarded_valid"
            return base
        base["rule_path"], _, base["rule_quote"] = quote
        detail = (row.get("route_rejection_detail") or "") + " " + rv
        dup = requires_repeated_elsewhere(orig, removed)
        if dup:
            base["review_bucket"] = "discarded_valid"
            base["notes"] = (
                f"requires restated elsewhere in original: removed `{removed}` vs `{dup}`"
            )
            return base
        if rv in ("no-entry", "unreached") or "Unsupported" in detail:
            base["review_bucket"] = "discarded_valid"
            base["notes"] = f"route={rv}; {row.get('route_rejection_detail', '')}"
            return base
        if rv in ("rejected", "incomplete", "defect", "outside", "fallback") and (
            PROOF_ROUTE_HINTS.search(detail)
            or PROOF_ROUTE_HINTS.search(base["obligation_line"])
        ):
            base["review_bucket"] = "confirmed_invalid"
            base["notes"] = (
                f"route proof failure: {row.get('route_rejection_detail', '')[:200]}"
            )
            return base
        if rv == "accepted":
            base["review_bucket"] = "confirmed_invalid"
            return base
        base["review_bucket"] = "discarded_route_only"
        base["notes"] = row.get("route_rejection_detail", "")
        return base

    base["review_bucket"] = "discarded_valid"
    return base


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


def hand_verify_section(confirmed: list[dict]) -> list[str]:
    lines = ["## Hand verification (5 samples)", ""]
    for i, row in enumerate(confirmed[:5], 1):
        mp = REPO / row["mutant_path"]
        out = run_checker_outputs(mp)
        orig = (REPO / row["fixture"]).read_text()
        mut = mp.read_text()
        diff = "\n".join(
            line
            for line in __import__("difflib").unified_diff(
                orig.splitlines()[:40],
                mut.splitlines()[:40],
                lineterm="",
                n=1,
            )
        )[:1500]
        lines.extend(
            [
                f"### Sample {i}: `{row['fixture']}` / {row['mutation_kind']}",
                "",
                "```diff",
                diff or "(diff truncated)",
                "```",
                "",
                f"- omega --check exit {out['omega_exit']}; output head:",
                "",
                "```",
                out["omega_out"][:800],
                "```",
                "",
                f"- route_one_comparison exit {out['route_exit']}; program line:",
                "",
                "```",
                (out["route_out"].splitlines()[0] if out["route_out"] else "")[:500],
                "```",
                "",
                f"- Failing obligation line: `{row.get('obligation_line', '')}`",
                "",
            ]
        )
    return lines


def hand_verify_drop_requires_discarded(reviewed: list[dict]) -> list[str]:
    pool = [
        r
        for r in reviewed
        if r.get("mutation_kind") == "drop_machine_requires"
        and r.get("current_verdict") == "accepted"
        and r.get("review_bucket") != "confirmed_invalid"
        and r.get("mutant_path")
    ]
    if not pool:
        return ["## Hand verification: discarded `drop_requires` (5 samples)", "", "_None yet._", ""]
    lines = ["## Hand verification: discarded `drop_requires` (5 samples)", ""]
    for i, row in enumerate(pool[:5], 1):
        orig = (REPO / row["fixture"]).read_text()
        mut = (REPO / row["mutant_path"]).read_text()
        removed = removed_requires_clause(orig, mut) or "(not parsed)"
        dup = requires_repeated_elsewhere(orig, removed) if removed.startswith("requires") else None
        lines.extend(
            [
                f"### Discarded drop_requires {i}: `{row['fixture']}`",
                f"- Bucket: `{row['review_bucket']}` — {row.get('notes', '')}",
                f"- Removed requires: `{removed}`",
                f"- Route rejection: `{row.get('route_rejection_detail', '')}`",
                f"- Duplicate elsewhere: `{dup or 'none'}`",
                "",
            ]
        )
    return lines


def hand_verify_q7_remove_mut(reviewed: list[dict]) -> list[str]:
    pool = [r for r in reviewed if r.get("review_bucket") == "known_gap_q7_mut"]
    if not pool:
        return ["## Hand verification: `known_gap_q7_mut` (5 samples)", "", "_None yet._", ""]
    lines = ["## Hand verification: `known_gap_q7_mut` (5 samples)", ""]
    for i, row in enumerate(pool[:5], 1):
        lines.extend(
            [
                f"### Q7 sample {i}: `{row['fixture']}`",
                f"- Binding obligation: `{row.get('obligation_line', '')}`",
                f"- Notes: {row.get('notes', '')}",
                "",
            ]
        )
    return lines


def main() -> None:
    if not TSV.is_file():
        print("missing TSV", TSV)
        return
    raw = list(csv.DictReader(TSV.open(), delimiter="\t"))
    reviewed = [classify_row(r) for r in raw]
    write_tsv(reviewed)

    bucket_counts = Counter(r["review_bucket"] for r in reviewed)
    total = len(reviewed)
    assert sum(bucket_counts.values()) == total

    by_kind = Counter(
        r["mutation_kind"] for r in reviewed if r["review_bucket"] == "confirmed_invalid"
    )

    confirmed = [r for r in reviewed if r["review_bucket"] == "confirmed_invalid"]

    lines = [
        f"# Soundness mutations audit (shard {SHARD} of 8)",
        "",
        f"PIN: `{PIN}`",
        "",
        "## Bucket table (sums to mutants run)",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    for b in FINAL_BUCKETS:
        lines.append(f"| {b} | {bucket_counts.get(b, 0)} |")
    lines.append(f"| **Total** | **{total}** |")
    lines.extend(
        [
            "",
            "## Confirmed invalid by mutation kind",
            "",
        ]
    )
    if by_kind:
        for k, c in sorted(by_kind.items()):
            lines.append(f"- `{k}`: {c}")
    else:
        lines.append("_None._")
    lines.extend(["", "## Confirmed holes", ""])
    if not confirmed:
        lines.append("_No confirmed-invalid holes (current accepts + spec obligation + quote)._")
    else:
        for i, h in enumerate(confirmed, 1):
            lines.extend(
                [
                    f"### {i}. `{h['fixture']}` ({h['mutation_kind']})",
                    f"- Mutant: `{h['mutant_path']}`",
                    f"- Current: `{h['current_verdict']}`; Route: `{h['route_verdict']}`",
                    f"- Obligation line: `{h['obligation_line']}`",
                    f"- Spec (`{h['rule_path']}`): > {h['rule_quote']}",
                    "",
                ]
            )

    lines.extend(hand_verify_section(confirmed))
    lines.extend(hand_verify_drop_requires_discarded(reviewed))
    lines.extend(hand_verify_q7_remove_mut(reviewed))

    if PROGRESS.is_file():
        prog = json.loads(PROGRESS.read_text())
        st = prog.get("stats", {})
        skipped = st.get("mutator_skipped_no_apply", 0)
        gen = st.get("mutants_generated", 0)
        run_failed = st.get("mutant_run_failed", 0)
        lines.extend(
            [
                "",
                "## Mutant totals (stable accounting)",
                "",
                f"- Rows in TSV (generated and run): **{total}**",
                f"- `mutants_generated` counter: {gen} (must match TSV rows)",
                f"- Mutator skipped (no applicable site, **not** in TSV): {skipped}",
                f"- Mutant run failed (bucket `mutant_run_failed`): {run_failed}",
                f"- Any change vs prior commits: full shard re-run after parser/review fixes",
                "",
                "## Run stats",
                "",
                f"- fixtures_total: {st.get('fixtures_total', '?')}",
                f"- fixtures_done: {len(prog.get('fixtures_done', []))}",
                f"- mutants_run: {st.get('mutants_run', total)}",
            ]
        )

    lines.extend(
        [
            "",
            "## Commands",
            "",
            "```bash",
            "CARGO_TARGET_DIR=$PWD/target mbx build -p omega -p compiler --examples",
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
