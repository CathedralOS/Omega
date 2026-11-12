#!/usr/bin/env python3
"""Reclassify existing shard-4.tsv (strict holes, not-applicable split) without re-checking."""

from __future__ import annotations

import csv
import importlib.util
import json
import re
import sys
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT / "shard-4.tsv"
STATS = OUT / "shard-4.stats.json"
PROGRESS = OUT / "shard-4.progress"
MUT_ROOT = OUT / "shard-4"


def slug(s: str) -> str:
    import hashlib

    h = hashlib.sha256(s.encode()).hexdigest()[:10]
    safe = re.sub(r"[^a-zA-Z0-9_-]+", "_", s)[:80]
    return f"{safe}_{h}"


def mutant_dir_exists(fixture_rel: str, kind: str) -> bool:
    d = MUT_ROOT / slug(f"{fixture_rel}_{kind}")
    return (d / "main.omg").is_file()


def parse_progress() -> list[tuple[str, str]]:
    attempts: list[tuple[str, str]] = []
    if not PROGRESS.exists():
        return attempts
    for ln in PROGRESS.read_text().splitlines():
        if not ln.startswith("fixture:"):
            continue
        body = ln[len("fixture:") :]
        if ":" not in body:
            continue
        rel, kind = body.rsplit(":", 1)
        attempts.append((rel, kind))
    return attempts


def load_finalize():
    path = OUT / "shard4_finalize.py"
    spec = importlib.util.spec_from_file_location("shard4_finalize", path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["shard4_finalize"] = mod
    spec.loader.exec_module(mod)
    return mod


CANONICAL_KIND = {
    "drop_mut_let": "remove_mut",
    "ref_to_mut_param": "shared_instead_of_mut",
    "mut_to_ref_param": "mut_ref_instead_of_shared",
    "widen_requires_lt": "widen_requires",
    "drop_requires": "drop_requires",
    "remove_transition_guard": "remove_transition_guard",
    "strip_and_guard": "strip_and_guard",
    "delete_divisor_guard": "delete_divisor_guard",
    "add_mut_let": "add_mut",
}


def requires_drop_site(orig: str, removed: list[str]) -> str:
    if not removed:
        return "unknown"
    lines = orig.splitlines()
    for i, ln in enumerate(lines):
        if not any(r.strip() in ln for r in removed):
            continue
        for j in range(i, max(-1, i - 8), -1):
            if j < 0:
                break
            if re.search(r"\bmachine\b", lines[j]) and "requires" in removed[0]:
                return "machine-header"
        return "body-or-data"
    return "unknown"


def strict_assess_extra(kind: str, orig: str, mut: str, base: dict) -> dict:
    """Tighten validity after base assess_validity_from_mutant."""
    out = dict(base)
    fin = load_finalize()
    if kind in ("remove_transition_guard", "strip_and_guard"):
        if fin.mutant_still_valid_heuristic(kind, orig, mut, ""):
            out["invalid"] = False
            out["valid_reason"] = (
                "dominating guard or remaining conjunct still enforces the fact in-machine"
            )
            return out
        if re.search(r"\bincorrect\s*\(\)|\bcrashes\b", mut):
            out["invalid"] = False
            out["valid_reason"] = "reachable incorrect()/crash path may discharge at run time"
            return out
        if kind == "remove_transition_guard":
            removed_m = re.search(r"transition\s+([^{;\n]+)\s*\{", orig)
            if removed_m:
                cond = re.sub(r"\s+", " ", removed_m.group(1).strip())
                if cond and cond in mut and "transition" in mut:
                    restated = next(
                        (
                            ln.strip()
                            for ln in mut.splitlines()
                            if "transition" in ln and cond in ln
                        ),
                        "",
                    )
                    if restated:
                        out["invalid"] = False
                        out["valid_reason"] = (
                            f"fix5 in-machine restatement | removed transition `{cond}` "
                            f"| still present: `{restated}`"
                        )
                        return out
        if not out.get("obligation_line"):
            out["invalid"] = False
            out["valid_reason"] = "no named violating operation after guard change"
    if kind == "delete_divisor_guard":
        if fin.mutant_still_valid_heuristic(kind, orig, mut, ""):
            out["invalid"] = False
            out["valid_reason"] = "division still guarded or absent"
            return out
        if re.search(r"\bincorrect\s*\(\)|\bcrashes\b", mut):
            out["invalid"] = False
            out["valid_reason"] = "integer policy may trap zero divisor at run time (not compile-time hole)"
            return out
        if re.search(r"\b[ui]\d+\b.*\s/\s|/\s*\d+[ui]", mut) and not re.search(
            r"\bembed\b|\bRat\b|\bInt\b.*mathematical", mut
        ):
            out["invalid"] = False
            out["valid_reason"] = (
                "typed integer `/` may use trapping policy; need proof-time nonzero quote"
            )
            return out
    if kind in ("ref_to_mut_param", "drop_mut_let"):
        out["q7_only"] = True
    if kind == "drop_requires" and out.get("invalid"):
        rem = fin.removed_requires(orig, mut)
        site = requires_drop_site(orig, rem)
        out["requires_site"] = site
    if kind in ("remove_transition_guard", "strip_and_guard") and out.get("invalid"):
        obl = (out.get("obligation_line") or "").split(":", 1)[-1].strip()
        if obl.startswith("transition") or not obl:
            out["invalid"] = False
            out["valid_reason"] = (
                "no spec-named body operation violates after guard change (transition-only)"
            )
    return out


def reclassify_row(fin, row: dict) -> dict:
    kind = row.get("mutation_kind", "")
    row["canonical_kind"] = CANONICAL_KIND.get(kind, kind)
    if not row.get("mutant_path"):
        row["stage"] = "not-applicable"
        row["bucket"] = ""
        row["no_mutation_reason"] = row.get("no_mutation_reason") or "kind not applicable"
        return row
    orig, mut = fin.read_pair(row["fixture"], row["mutant_path"])
    base = fin.assess_validity_from_mutant(kind, orig, mut)
    base = strict_assess_extra(kind, orig, mut, base)
    row["obligation_line"] = base.get("obligation_line", "")
    row["spec_rule_file"] = base.get("spec_rule_file", "")
    row["spec_quote"] = base.get("spec_quote", "")
    row["validity_reason"] = base.get("valid_reason", "")
    ac_by = fin.accepted_by_column(row.get("current_verdict", ""), row.get("route_verdict", ""))
    if base["invalid"]:
        if base.get("q7_only") and ac_by:
            row["bucket"] = "known-gap-q7-mut"
            row["accepted_by"] = ""
        elif ac_by:
            ok, why = fin.fix4_check_time_hole(kind, orig, mut)
            if ok and base.get("obligation_line") and base.get("spec_quote"):
                row["bucket"] = "confirmed-hole"
                row["accepted_by"] = ac_by
                row["check_time_note"] = why
                row["no_trap_evidence"] = why
                if kind == "drop_requires":
                    rem = fin.removed_requires(orig, mut)
                    sub, note = fin.probe_modular_entry(
                        row["fixture"], row["mutant_path"], rem
                    )
                    row["hole_subtype"] = sub
                    row["probe_note"] = note
                    if sub == "violating-caller-accepted":
                        row["bucket"] = "discarded-valid"
                        row["validity_reason"] = note
                row["minimal_diff"] = fin.minimal_diff(orig, mut)[:2000]
            else:
                cur, route = row.get("current_verdict", ""), row.get("route_verdict", "") or ""
                row["bucket"] = (
                    "disagreement-valid"
                    if fin.checkers_disagree(cur, route)
                    else "rejected-both"
                    if cur == "rejected" and route == "rejected"
                    else "rejected-current-other"
                )
                row["validity_reason"] = why or base.get("valid_reason") or "missing quote/obligation"
        else:
            cur, route = row.get("current_verdict", ""), row.get("route_verdict", "") or ""
            if cur == "rejected" and route == "rejected":
                row["bucket"] = "rejected-both"
            elif cur == "rejected" and fin.route_is_outside(route):
                row["bucket"] = "rejected-current-route-outside"
            elif cur == "rejected" and not route:
                row["bucket"] = "rejected-current-no-route"
            elif cur == "rejected":
                row["bucket"] = "rejected-current-other"
            else:
                row["bucket"] = "discarded-valid"
    elif fin.route_is_outside(row.get("route_verdict", "")):
        row["bucket"] = "valid-route-outside"
    elif fin.checkers_disagree(row.get("current_verdict", ""), row.get("route_verdict", "")):
        row["bucket"] = "disagreement-valid"
    elif row.get("current_verdict") == "accepted" and row.get("route_verdict") == "accepted":
        row["bucket"] = "discarded-valid"
    elif row.get("current_verdict") == "rejected" and row.get("route_verdict") == "rejected":
        row["bucket"] = "rejected-both"
    else:
        row["bucket"] = "discarded-valid"
    row["hand_check"] = row["bucket"]
    return row


def fixture_partition(rows: list[dict]) -> dict:
    orig = {r["fixture"]: r for r in rows if r["stage"] == "original"}
    has_mut = set(
        r["fixture"] for r in rows if r.get("stage") == "mutant" and r.get("mutant_path")
    )
    not_acc = {f for f, r in orig.items() if r.get("current_verdict") != "accepted"}
    mutated = has_mut
    no_mut = set(orig) - mutated
    no_mut_ok = no_mut - not_acc
    return {
        "fixtures_total": len(orig),
        "original_not_accepted": len(not_acc),
        "no_mutation_accepted_baseline": len(no_mut_ok),
        "mutated": len(mutated),
    }


def main() -> int:
    fin = load_finalize()
    rows = list(csv.DictReader(TSV.open(), delimiter="\t"))
    originals = [r for r in rows if r["stage"] == "original"]
    na_old = [r for r in rows if r["stage"] == "no-mutation"]
    mutants_in = [r for r in rows if r["stage"] == "mutant"]

    reclassified: list[dict] = []
    for r in mutants_in:
        reclassified.append(reclassify_row(fin, dict(r)))

    have_attempt = {(r["fixture"], r.get("mutation_kind", "")) for r in mutants_in}
    for rel, kind in parse_progress():
        if (rel, kind) in have_attempt or mutant_dir_exists(rel, kind):
            continue
        reclassified.append(
            {
                "fixture": rel,
                "tier": rel.split("/")[2],
                "stage": "not-applicable",
                "mutation_kind": kind,
                "mutant_path": "",
                "canonical_kind": CANONICAL_KIND.get(kind, kind),
                "no_mutation_reason": f"mutation `{kind}` not applicable to source",
                "bucket": "",
                "hand_check": "not-applicable",
            }
        )

    not_applicable = [r for r in reclassified if r["stage"] == "not-applicable"]
    mutants = [r for r in reclassified if r["stage"] == "mutant"]
    buckets = Counter(r["bucket"] for r in mutants)

    drop_sites = Counter()
    for r in mutants:
        if r.get("mutation_kind") != "drop_requires" or not r.get("mutant_path"):
            continue
        o, m = fin.read_pair(r["fixture"], r["mutant_path"])
        rem = fin.removed_requires(o, m)
        drop_sites[requires_drop_site(o, rem)] += 1

    part = fixture_partition(originals + reclassified)
    assert (
        part["original_not_accepted"]
        + part["no_mutation_accepted_baseline"]
        + part["mutated"]
        == part["fixtures_total"]
    ), part

    kind_counts = Counter(r.get("canonical_kind") or r.get("mutation_kind") for r in mutants)
    for ck in (
        "drop_requires",
        "widen_requires",
        "remove_mut",
        "shared_instead_of_mut",
        "mut_ref_instead_of_shared",
        "remove_transition_guard",
        "strip_and_guard",
        "delete_divisor_guard",
    ):
        kind_counts.setdefault(ck, 0)

    header = list(rows[0].keys()) if rows else []
    for extra in ("canonical_kind", "no_trap_evidence", "requires_site"):
        if extra not in header:
            header.append(extra)

    with TSV.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=header, delimiter="\t", extrasaction="ignore")
        w.writeheader()
        w.writerows(originals)
        w.writerows(na_old)
        w.writerows(mutants)
        w.writerows(not_applicable)

    stats = json.loads(STATS.read_text()) if STATS.exists() else {}
    stats.update(
        {
            "fixture_partition": part,
            "not_applicable_attempts": len(not_applicable),
            "mutant_rows_with_path": len(mutants),
            "mutation_attempts_progress": len(parse_progress()),
            "mutant_buckets": dict(buckets),
            "canonical_kind_counts": dict(kind_counts),
            "drop_requires_site_split": dict(drop_sites),
        }
    )
    STATS.write_text(json.dumps(stats, indent=2) + "\n")

    holes = [r for r in mutants if r["bucket"] == "confirmed-hole"]
    print("fixtures", part)
    print("not_applicable", len(not_applicable), "mutants", len(mutants), "sum buckets", sum(buckets.values()))
    print("buckets", dict(buckets))
    print("confirmed", len(holes), dict(Counter(r.get("canonical_kind") for r in holes)))
    print("drop_requires sites", dict(drop_sites))
    return 0


if __name__ == "__main__":
    sys.exit(main())
