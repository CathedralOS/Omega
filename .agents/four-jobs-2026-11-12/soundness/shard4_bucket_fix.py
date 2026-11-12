#!/usr/bin/env python3
"""Apply bucket naming fixes (rejected-both vs route-skipped) and write audit sections."""

from __future__ import annotations

import csv
import importlib.util
import io
import json
import subprocess
import sys
from collections import Counter, defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT / "shard-4.tsv"
STATS = OUT / "shard-4.stats.json"
OLD_PIN = "c0a4894a0c4"

CANONICAL_KIND = {
    "drop_mut_let": "remove_mut",
    "ref_to_mut_param": "shared_instead_of_mut",
    "mut_to_ref_param": "mut_ref_instead_of_shared",
    "widen_requires_lt": "widen_requires",
}


def load_finalize():
    path = OUT / "shard4_finalize.py"
    spec = importlib.util.spec_from_file_location("shard4_finalize", path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["shard4_finalize"] = mod
    spec.loader.exec_module(mod)
    return mod


def load_reclassify():
    path = OUT / "shard4_reclassify.py"
    spec = importlib.util.spec_from_file_location("shard4_reclassify", path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["shard4_reclassify"] = mod
    spec.loader.exec_module(mod)
    return mod


def judicial_route(route: str) -> bool:
    return route in {"accepted", "rejected", "incomplete", "fallback", "defect"}


def fix_bucket(row: dict, fin, rc) -> dict:
    """Recompute bucket with strict validity and corrected rejection labels."""
    kind = row.get("mutation_kind", "")
    cur = row.get("current_verdict", "")
    route = row.get("route_verdict", "") or ""
    if not row.get("mutant_path"):
        row["bucket"] = ""
        return row
    orig, mut = fin.read_pair(row["fixture"], row["mutant_path"])
    base = fin.assess_validity_from_mutant(kind, orig, mut)
    base = rc.strict_assess_extra(kind, orig, mut, base)
    row["validity_reason"] = base.get("valid_reason", "") or row.get("validity_reason", "")
    ac_by = fin.accepted_by_column(cur, route)
    invalid = base.get("invalid", False)

    if cur == "timeout" or route == "timeout":
        row["bucket"] = "timeout"
        return row
    if invalid:
        if base.get("q7_only") and ac_by:
            row["bucket"] = "known-gap-q7-mut"
            row["accepted_by"] = ""
        elif ac_by:
            ok, why = fin.fix4_check_time_hole(kind, orig, mut)
            if ok and base.get("obligation_line") and base.get("spec_quote"):
                row["bucket"] = "confirmed-hole"
                row["accepted_by"] = ac_by
                row["no_trap_evidence"] = why
            else:
                row["bucket"] = (
                    "disagreement-valid"
                    if fin.checkers_disagree(cur, route)
                    else "rejected-both"
                    if cur == "rejected" and route == "rejected"
                    else "rejected-current-other"
                )
        elif cur == "rejected" and route == "rejected":
            row["bucket"] = "rejected-both"
        elif cur == "rejected" and fin.route_is_outside(route):
            row["bucket"] = "rejected-current-route-outside"
        elif cur == "rejected" and not route:
            row["bucket"] = "rejected-current-no-route"
        elif cur == "rejected":
            row["bucket"] = "rejected-current-other"
        else:
            row["bucket"] = "rejected-current-other"
    elif fin.route_is_outside(route):
        row["bucket"] = "valid-route-outside"
    elif fin.checkers_disagree(cur, route):
        row["bucket"] = "disagreement-valid"
    elif cur == "accepted" and route == "accepted":
        row["bucket"] = "discarded-valid"
    elif cur == "rejected" and route == "rejected":
        row["bucket"] = "rejected-both"
    else:
        row["bucket"] = "discarded-valid"
    row["hand_check"] = row["bucket"]
    row["canonical_kind"] = CANONICAL_KIND.get(kind, kind)
    return row


TRANSITION_RULES: dict[tuple[str, str], str] = {
    ("discarded-valid", "valid-route-outside"): (
        "Strict validity: mutant read valid; route is outside/no-entry (fix3 route-only non-verdict)."
    ),
    ("discarded-valid", "rejected-current-no-route"): (
        "Strict invalid mutant; current checker rejects; route never run (runner skips route on reject)."
    ),
    ("discarded-valid", "disagreement-valid"): (
        "Checker disagreement after strict validity reassessment."
    ),
    ("confirmed-hole", "valid-route-outside"): (
        "Hole demoted: guard/divisor/param lacks compile-time invalidity or run-time trap discharge."
    ),
    ("confirmed-hole", "discarded-valid"): (
        "Hole demoted: missing spec quote, obligation, or no-trap evidence."
    ),
    ("confirmed-hole", "known-gap-q7-mut"): (
        "Ownership/Q7 mutability gap (remove_mut / shared_instead_of_mut) per lead notes."
    ),
    ("rejected-both", "rejected-current-no-route"): (
        "Rename: rejected-both requires route=rejected; empty route is route-skipped not dual rejection."
    ),
    ("discarded-valid", "rejected-both"): (
        "Superseded rename to rejected-current-no-route (same rule)."
    ),
}


def old_tsv_buckets() -> dict[tuple[str, str, str], str]:
    raw = subprocess.check_output(
        ["git", "show", f"{OLD_PIN}:.agents/four-jobs-2026-11-12/soundness/shard-4.tsv"],
        cwd=REPO,
    )
    rows = csv.DictReader(io.StringIO(raw.decode()), delimiter="\t")
    return {
        (r["fixture"], r["mutation_kind"], r["mutant_path"]): r.get("bucket", "")
        for r in rows
        if r.get("stage") == "mutant" and r.get("mutant_path")
    }


def spot_checks(
    rows: list[dict], old: dict[tuple[str, str, str], str], flow: tuple[str, str], n: int = 5
) -> list[str]:
    a, b = flow
    picks = [
        r
        for r in rows
        if old.get((r["fixture"], r["mutation_kind"], r["mutant_path"])) == a
        and r.get("bucket") == b
    ][:n]
    lines: list[str] = []
    rule = TRANSITION_RULES.get(flow, "Strict reclassify rule (see shard4_reclassify.py).")
    for r in picks:
        lines.append(
            f"- `{r['fixture']}` / `{r['mutation_kind']}`: {rule} "
            f"(cur={r['current_verdict']}, route={r.get('route_verdict') or '∅'})"
        )
    return lines


def verify_rejected_both(rows: list[dict]) -> tuple[int, list[dict]]:
    rb = [r for r in rows if r.get("bucket") == "rejected-both"]
    bad = [r for r in rb if r.get("current_verdict") != "rejected" or r.get("route_verdict") != "rejected"]
    return len(bad), bad


def main() -> int:
    fin = load_finalize()
    rc = load_reclassify()
    rows = list(csv.DictReader(TSV.open(), delimiter="\t"))
    originals = [r for r in rows if r["stage"] == "original"]
    na_fixture = [r for r in rows if r["stage"] == "no-mutation"]
    not_app = [r for r in rows if r["stage"] == "not-applicable"]
    mut_in = [r for r in rows if r["stage"] == "mutant"]

    fixed = [fix_bucket(dict(r), fin, rc) for r in mut_in if r.get("mutant_path")]
    old = old_tsv_buckets()
    trans = Counter()
    for r in fixed:
        key = (r["fixture"], r["mutation_kind"], r["mutant_path"])
        ob = old.get(key, "")
        nb = r.get("bucket", "")
        if ob and ob != nb:
            trans[(ob, nb)] += 1

    bad_rb, _ = verify_rejected_both(fixed)
    assert bad_rb == 0, f"rejected-both failures {bad_rb}"

    # kind × bucket
    cross: dict[str, Counter[str]] = defaultdict(Counter)
    kind_totals = Counter()
    for r in fixed:
        ck = r.get("canonical_kind") or CANONICAL_KIND.get(r["mutation_kind"], r["mutation_kind"])
        kind_totals[ck] += 1
        cross[ck][r["bucket"]] += 1

    header = list(rows[0].keys()) if rows else []
    for extra in ("canonical_kind",):
        if extra not in header:
            header.append(extra)

    with TSV.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=header, delimiter="\t", extrasaction="ignore")
        w.writeheader()
        w.writerows(originals)
        w.writerows(na_fixture)
        w.writerows(fixed)
        w.writerows(not_app)

    stats = json.loads(STATS.read_text()) if STATS.exists() else {}
    stats["bucket_transition_from_c0a4894"] = {
        "matrix": {f"{a} -> {b}": c for (a, b), c in sorted(trans.items())},
        "rules": {f"{a} -> {b}": TRANSITION_RULES.get((a, b), "") for a, b in trans},
        "rejected_both_verification_failures": bad_rb,
        "kind_totals": dict(kind_totals),
        "kind_bucket_cross": {k: dict(v) for k, v in sorted(cross.items())},
    }
    STATS.write_text(json.dumps(stats, indent=2) + "\n")

    audit_path = OUT / "shard4_audit_report.md"
    lines = [
        "# Shard 4 audit supplement",
        "",
        "## Bucket transition (c0a4894 → post-fix)",
        "",
        "| Old | New | Count | Rule |",
        "| --- | --- | ---: | --- |",
    ]
    for (a, b), c in trans.most_common():
        lines.append(f"| `{a}` | `{b}` | {c} | {TRANSITION_RULES.get((a, b), 'reclassify')} |")
    lines.extend(
        [
            "",
            f"**rejected-both strict-check failures after fix:** {bad_rb}",
            "",
            "## Spot checks (5 per moved flow)",
            "",
        ]
    )
    for flow in (
        ("discarded-valid", "valid-route-outside"),
        ("discarded-valid", "rejected-current-no-route"),
        ("confirmed-hole", "valid-route-outside"),
    ):
        b = flow[1]
        actual_flow = flow
        if flow[1] == "rejected-current-no-route":
            # map from old matrix name
            pass
        n = trans.get((flow[0], b), 0) or trans.get((flow[0], "rejected-both"), 0)
        lines.append(f"### {flow[0]} → {b} ({n} rows)")
        lines.append("")
        # use actual new bucket in picks
        picks_flow = (flow[0], b)
        if not trans.get(picks_flow) and flow[0] == "discarded-valid" and b == "rejected-current-no-route":
            picks_flow = ("discarded-valid", "rejected-current-no-route")
        if not trans.get(picks_flow):
            picks_flow = ("discarded-valid", "rejected-both")
            if not trans.get(picks_flow):
                picks_flow = ("rejected-both", "rejected-current-no-route")
        lines.extend(spot_checks(fixed, old, picks_flow, 5))
        lines.append("")

    lines.extend(["## Kind totals (canonical)", "", "| Kind | Count |", "| --- | ---: |"])
    for k, v in sorted(kind_totals.items()):
        lines.append(f"| `{k}` | {v} |")
    lines.append(f"| **Sum** | **{sum(kind_totals.values())}** |")
    lines.extend(["", "## Kind × bucket", ""])
    buckets = sorted({b for c in cross.values() for b in c})
    lines.append("| Kind | " + " | ".join(f"`{b}`" for b in buckets) + " |")
    lines.append("| --- | " + " | ".join("---:" for _ in buckets) + " |")
    for k in sorted(cross):
        lines.append("| `" + k + "` | " + " | ".join(str(cross[k].get(b, 0)) for b in buckets) + " |")

    audit_path.write_text("\n".join(lines) + "\n")
    print("buckets", Counter(r["bucket"] for r in fixed))
    print("rejected-both bad", bad_rb, "kind sum", sum(kind_totals.values()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
