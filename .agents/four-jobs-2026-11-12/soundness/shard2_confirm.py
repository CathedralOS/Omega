#!/usr/bin/env python3
"""Post-process shard TSV (fix3 + fix4)."""
from __future__ import annotations

import csv
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from shard2_review import (
    ROUTE_NON_VERDICT,
    SPEC_RULES,
    acceptance_mechanism,
    accepted_by,
    current_rejects,
    is_known_q7_gap,
    minimal_mutant_diff,
    probe_drop_requires_subtype,
    read_mutant_validity,
    removed_requires_clause,
    route_rejects,
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
    "valid_route_outside",
    "discarded_valid",
    "rejected_both",
    "timeout",
    "failed_run",
)

CHECK_TIME_RULE_KINDS = frozenset(
    {
        "remove_mut",
        "drop_machine_requires",
        "widen_requires_lt",
        "ref_to_mut_ref",
        "mut_ref_to_ref",
    }
)


def checker_accepts(cv: str, rv: str) -> bool:
    return cv == "accepted" or rv == "accepted"


def both_reject(cv: str, rv: str) -> bool:
    if checker_accepts(cv, rv):
        return False
    if current_rejects(cv) and (route_rejects(rv) or rv in ROUTE_NON_VERDICT):
        return True
    if cv in ("timeout", "error") or rv in ("timeout", "error"):
        return True
    return current_rejects(cv) and rv in ("unknown", "error")


def rule_heading(kind: str, rule_path: str) -> str:
    for key, (path, heading, _) in SPEC_RULES.items():
        if path == rule_path:
            return heading
    if kind == "remove_mut":
        return "Owned local mutability"
    return ""


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
        "rule_heading": "",
        "acceptance_mechanism": "",
        "hole_subtype": "",
        "probe_note": "",
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
    base["rule_heading"] = rule_heading(kind, rpath)

    if kind == "remove_mut" and invalid and oblig:
        bn = binding_from_remove(orig)
        if bn:
            q7, q7s = is_known_q7_gap(orig_path, bn, oblig)
            if q7 and checker_accepts(cv, rv):
                mech = acceptance_mechanism(mut_path)
                base["acceptance_mechanism"] = mech
                base["review_bucket"] = "known_gap_q7_mut"
                base["notes"] = f"Q7: {q7s}"
                base["rule_path"] = "wiki/spec/language/ownership.md"
                base["rule_heading"] = "Owned local mutability"
                base["rule_quote"] = (
                    "modifying an established ordinary local's owned storage requires a `mut` binding"
                )
                return base

    if not invalid:
        if rv in ROUTE_NON_VERDICT:
            base["review_bucket"] = "valid_route_outside"
            base["notes"] = f"valid mutant; route={rv} is not a checker verdict"
            return base
        if (cv == "accepted" and route_rejects(rv)) or (
            rv == "accepted" and current_rejects(cv)
        ):
            base["review_bucket"] = "disagreement_valid"
            base["notes"] = f"valid mutant; current={cv} route={rv}"
            return base
        if checker_accepts(cv, rv):
            base["review_bucket"] = "discarded_valid"
            return base

    if invalid and checker_accepts(cv, rv):
        mech = acceptance_mechanism(mut_path)
        base["acceptance_mechanism"] = mech
        if mech == "runtime_trap":
            base["review_bucket"] = "discarded_valid"
            base["validity_reason"] += (
                "; fix4: accepting checker appears to route violation to runtime trap"
            )
            return base
        if kind not in CHECK_TIME_RULE_KINDS or not rquote:
            base["review_bucket"] = "discarded_valid"
            base["validity_reason"] += "; fix4: no check-time spec quote"
            return base
        if kind in ("drop_machine_requires", "widen_requires_lt"):
            removed = removed_requires_clause(orig, mut) or ""
            if removed:
                sub, pnote = probe_drop_requires_subtype(mut_path, orig_path, removed)
                base["hole_subtype"] = sub
                base["probe_note"] = pnote
        base["review_bucket"] = "confirmed_hole"
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
        "rule_heading",
        "rule_quote",
        "acceptance_mechanism",
        "hole_subtype",
        "probe_note",
        "notes",
        "seconds",
    ]
    with TSV.open("w") as f:
        f.write("\t".join(cols) + "\n")
        for r in rows:
            f.write("\t".join(str(r.get(c, "")) for c in cols) + "\n")


def reconcile_section(total_rows: int, reviewed: list[dict]) -> list[str]:
    prog = json.loads(PROGRESS.read_text()) if PROGRESS.is_file() else {}
    st = prog.get("stats", {})
    fx_total = st.get("fixtures_total", 1130)
    fx_done = len(prog.get("fixtures_done", []))
    no_mut = st.get("fixtures_no_mutation", len(prog.get("fixtures_no_mutation", [])))
    mutated = st.get("fixtures_mutated", fx_done - no_mut - st.get("fixtures_skipped_original_fails", 0))
    skipped = st.get("fixtures_skipped_original_fails", 0)
    by_fx = Counter(r["fixture"] for r in reviewed)
    multi = [fx for fx, n in by_fx.items() if n > 1]
    lines = [
        "## Fixture / mutant reconciliation (fix4)",
        "",
        f"| | Count |",
        f"| --- | ---: |",
        f"| Fixtures in shard | {fx_total} |",
        f"| Fixtures processed | {fx_done} |",
        f"| Original check failed (skipped) | {skipped} |",
        f"| No-mutation fixtures (all mutators N/A) | {no_mut} |",
        f"| Mutated fixtures (≥1 mutant row) | {mutated} |",
        f"| Mutant rows (TSV) | {total_rows} |",
        "",
    ]
    if multi:
        lines.append("Fixtures with more than one mutant row:")
        for fx in sorted(multi)[:50]:
            lines.append(f"- `{fx}` ({by_fx[fx]} rows)")
        if len(multi) > 50:
            lines.append(f"- … and {len(multi) - 50} more")
    else:
        lines.append("_No fixture with multiple rows in this slice._")
    lines.append("")
    return lines


def confirmed_hole_writeup(confirmed: list[dict]) -> list[str]:
    lines = ["## Confirmed holes (fix4 airtight)", ""]
    if not confirmed:
        lines.append("_None._")
        return lines
    for i, h in enumerate(confirmed, 1):
        mp = REPO / h["mutant_path"]
        orig = (REPO / h["fixture"]).read_text()
        diff = minimal_mutant_diff(orig, mp.read_text())
        lines.extend(
            [
                f"### {i}. `{h['fixture']}` ({h['mutation_kind']}, accepted_by={h['accepted_by']})",
                "",
                f"**(a) Check-time spec** (`{h['rule_path']}` › {h.get('rule_heading', '')})",
                "",
                f"> {h['rule_quote']}",
                "",
                f"**(b) Acceptance mechanism:** `{h.get('acceptance_mechanism', '')}` "
                f"(must not be runtime-trap-only; route hint `{h.get('route_rejection_detail', '')[:120]}`)",
                "",
                "**(c) Minimal diff:**",
                "",
                "```diff",
                diff,
                "```",
                "",
                f"- Obligation line: `{h['obligation_line']}`",
                f"- Mutant: `{h['mutant_path']}`",
                f"- hole_subtype: `{h.get('hole_subtype', '')}`; probe: {h.get('probe_note', '')}",
                "",
            ]
        )
    return lines


def hand_block(title: str, rows: list[dict], n: int = 5) -> list[str]:
    if not rows:
        return [f"## {title}", "", "_None._", ""]
    out = [f"## {title}", ""]
    for i, row in enumerate(rows[:n], 1):
        out.append(
            f"### {i}. `{row['fixture']}` / {row['mutation_kind']} / {row['review_bucket']}"
        )
        out.append(f"- Validity read: {row.get('validity_reason', '')}")
        out.append(f"- accepted_by: `{row.get('accepted_by')}`; route: `{row['route_verdict']}`")
        out.append("")
    return out


def main() -> None:
    if not TSV.is_file():
        print("missing", TSV)
        return
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
        "## Bucket table",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    for b in FINAL_BUCKETS:
        lines.append(f"| {b} | {bucket_counts.get(b, 0)} |")
    lines.append(f"| **Total mutants** | **{total}** |")
    lines.extend(reconcile_section(total, reviewed))
    lines.extend(["## Confirmed holes by kind", ""])
    for k, c in sorted(by_kind.items()):
        lines.append(f"- `{k}`: {c}")
    lines.extend(["", "## Confirmed holes by accepted_by", ""])
    for k, c in sorted(by_acc.items()):
        lines.append(f"- `{k}`: {c}")
    lines.extend(confirmed_hole_writeup(confirmed))
    lines.extend(
        hand_block(
            "Hand check: 5 discarded-valid",
            [r for r in reviewed if r["review_bucket"] == "discarded_valid"],
        )
    )
    lines.extend(
        hand_block(
            "Hand check: 5 valid-route-outside",
            [r for r in reviewed if r["review_bucket"] == "valid_route_outside"],
        )
    )
    lines.extend(
        hand_block(
            "Hand check: 5 known-gap-q7-mut",
            [r for r in reviewed if r["review_bucket"] == "known_gap_q7_mut"],
        )
    )
    lines.append("## Commands")
    lines.append("")
    lines.append("```bash")
    lines.append("PYTHONUNBUFFERED=1 python3 .agents/four-jobs-2026-11-12/soundness/shard2_mutate_run.py")
    lines.append("python3 .agents/four-jobs-2026-11-12/soundness/shard2_confirm.py")
    lines.append("```")

    MD.write_text("\n".join(lines) + "\n")
    print(bucket_counts)


if __name__ == "__main__":
    main()
