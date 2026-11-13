#!/usr/bin/env python3
"""Shard 2 run 2: re-bucket, canonical kinds, Q7, prune, integrity report."""
from __future__ import annotations

import csv
import hashlib
import json
import re
import shutil
import subprocess
import tarfile
from collections import Counter, defaultdict
from pathlib import Path

import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from shard2_canonical import CANONICAL_KINDS, LEGACY_MAP, canonical_kind
from shard2_mutate_run import (
    ENV,
    OMEGA,
    ROUTE,
    check_both,
    current_verdict,
    fixture_list,
    tier,
)
from shard2_confirm import (
    binding_from_remove,
    checker_accepts,
    rule_heading as _rule_heading,
)
from shard2_review import (
    ROUTE_NON_VERDICT,
    accepted_by,
    current_rejects,
    minimal_mutant_diff,
    probe_drop_requires_subtype,
    read_mutant_validity,
    removed_requires_clause,
    route_rejects,
    verbatim_spec_quote,
    q7_binding_for_row,
    is_known_q7_gap,
    acceptance_mechanism,
)

REPO = Path(__file__).resolve().parents[3]
PIN = "0835b3aaf803ed97f905be42d94727ff58459263"
SHARD = 2
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT / f"shard-{SHARD}.tsv"
MD = OUT / f"shard-{SHARD}.md"
PROGRESS = OUT / f"shard-{SHARD}.progress.json"
MUT_ROOT = OUT / f"shard-{SHARD}"
ARCHIVE = OUT / f"shard-{SHARD}-mutants-pruned.tar.gz"
Q7_INDEX = OUT / "q7_shard_index.json"

BUCKETS = (
    "confirmed-hole",
    "known-gap-q7-mut",
    "disagreement-valid",
    "valid-route-outside",
    "rejected-current-route-outside",
    "discarded-valid",
    "rejected-both",
    "timeout",
    "failed-run",
)

HARNESS_MARKERS = (
    "needs an accepted program-entry binding",
    "ProgramEntry",
    "package review",
    "OMEGA_LIBRARY_ROOT",
    "could not resolve package",
)

CHECK_TIME_KINDS = frozenset(
    {
        "remove_mut",
        "drop_requires",
        "widen_requires",
        "mut_ref_instead_of_shared",
        "shared_instead_of_mut",
    }
)

UNVERIFIED_SUBTYPES = frozenset({"probe-not-built", "inferred-entry", "unchecked"})


def both_reject(cv: str, rv: str) -> bool:
    if checker_accepts(cv, rv):
        return False
    if current_rejects(cv) and (route_rejects(rv) or rv in ROUTE_NON_VERDICT):
        return True
    if cv in ("timeout", "error") or rv in ("timeout", "error"):
        return True
    return current_rejects(cv) and rv in ("unknown", "error")


def is_harness_error(text: str) -> bool:
    t = text.lower()
    return any(m.lower() in t for m in HARNESS_MARKERS)


def rescan_original_skips() -> tuple[list[dict], list[dict]]:
    """Return (genuine original-not-accepted, harness misclassified)."""
    fixtures = fixture_list()
    in_tsv = set()
    if TSV.is_file():
        for row in csv.DictReader(TSV.open(), delimiter="\t"):
            in_tsv.add(row["fixture"])
    genuine: list[dict] = []
    harness: list[dict] = []
    for fx in fixtures:
        if fx in in_tsv:
            continue
        path = REPO / fx
        cv, rv, detail, raw = check_both(path)
        rec = {
            "fixture": fx,
            "current_verdict": cv,
            "route_verdict": rv,
            "diagnostic": detail or raw[:800],
        }
        if cv == "accepted":
            harness.append({**rec, "note": "now accepted; was skipped without rows"})
        elif is_harness_error(rec["diagnostic"] + raw):
            harness.append(rec)
        else:
            genuine.append(rec)
    return genuine, harness


def classify_row(row: dict, q7_idx: dict) -> dict:
    legacy_kind = row.get("mutation_kind", "")
    kind = canonical_kind(legacy_kind)
    cv = row.get("current_verdict", "")
    rv = row.get("route_verdict", "")
    run_disp = row.get("run_disposition", "")
    base = {**row, "mutation_kind": kind, "legacy_kind": legacy_kind}
    base["review_bucket"] = ""
    base["accepted_by"] = accepted_by(cv, rv)

    # Fix corrupted run_disposition from prior confirm
    if "confirmed_hole" in run_disp or "valid_route" in run_disp:
        run_disp = (
            "checker_accepts"
            if checker_accepts(cv, rv)
            else ("rejected_both" if both_reject(cv, rv) else "run_failed")
        )
        base["run_disposition"] = run_disp

    if run_disp == "run_failed":
        base["review_bucket"] = "failed-run"
        return base
    if run_disp == "timeout" or cv == "timeout" or rv == "timeout":
        base["review_bucket"] = "timeout"
        return base

    if current_rejects(cv) and rv in ROUTE_NON_VERDICT:
        base["review_bucket"] = "rejected-current-route-outside"
        base["validity_reason"] = f"current={cv}, route={rv} (non-verdict)"
        return base

    if both_reject(cv, rv):
        base["review_bucket"] = "rejected-both"
        return base

    mutant_path = (row.get("mutant_path") or "").strip()
    if not mutant_path:
        base["review_bucket"] = "rejected-both"
        base["validity_reason"] = "mutant not retained"
        return base

    orig_path = REPO / row["fixture"]
    mut_path = REPO / mutant_path
    if not mut_path.is_file() and mutant_path:
        base["review_bucket"] = "discarded-valid"
        base["validity_reason"] = "mutant archived after classification"
        return base

    orig = orig_path.read_text()
    mut = mut_path.read_text()
    read_kind = legacy_kind if legacy_kind in LEGACY_MAP or legacy_kind in (
        "drop_machine_requires",
        "widen_requires_lt",
        "remove_mut",
        "ref_to_mut_ref",
        "mut_ref_to_ref",
        "drop_machine_ensures",
    ) else kind
    invalid, reason, oblig, rpath, rquote = read_mutant_validity(
        orig, mut, read_kind, orig_path
    )
    heading = _rule_heading(read_kind, rpath)
    if rpath and heading:
        rquote = verbatim_spec_quote(rpath, heading) or rquote

    base.update(
        {
            "validity_reason": reason,
            "obligation_line": oblig,
            "rule_path": rpath,
            "rule_heading": heading,
            "rule_quote": rquote,
        }
    )

    q7_kinds = ("remove_mut", "mut_ref_instead_of_shared", "shared_instead_of_mut")
    if kind in q7_kinds and checker_accepts(cv, rv):
        binding = q7_binding_for_row(orig, mut, kind) or binding_from_remove(orig) or ""
        q7_hit, q7_snip = False, ""
        if binding and row["fixture"] in q7_idx:
            for hit in q7_idx[row["fixture"]]:
                if hit["binding"] == binding:
                    q7_hit, q7_snip = True, hit.get("snippet", "")
                    break
        if not q7_hit:
            q7_hit, q7_snip = is_known_q7_gap(orig_path, binding or "", oblig or mut)
        if q7_hit:
            mech = (row.get("acceptance_mechanism") or "").strip()
            if not mech:
                mech = acceptance_mechanism(mut_path)
            base["acceptance_mechanism"] = mech
            base["review_bucket"] = "known-gap-q7-mut"
            base["notes"] = f"Q7 list: {q7_snip[:200]}"
            path = "wiki/spec/language/ownership.md"
            base["rule_path"] = path
            base["rule_heading"] = "Owned local mutability"
            base["rule_quote"] = verbatim_spec_quote(path, "Owned local mutability")
            return base

    if not invalid:
        if rv in ROUTE_NON_VERDICT:
            base["review_bucket"] = "valid-route-outside"
            base["notes"] = f"valid mutant; route={rv}"
            return base
        if (cv == "accepted" and route_rejects(rv)) or (
            rv == "accepted" and current_rejects(cv)
        ):
            base["review_bucket"] = "disagreement-valid"
            return base
        if checker_accepts(cv, rv):
            base["review_bucket"] = "discarded-valid"
            return base

    if invalid and not checker_accepts(cv, rv):
        base["review_bucket"] = "rejected-both"
        return base

    if invalid and checker_accepts(cv, rv):
        if rv in ROUTE_NON_VERDICT and cv == "accepted":
            base["review_bucket"] = "valid-route-outside"
            base["notes"] = (
                "invalid under spec read but only route non-verdict accepts; "
                "not a confirmed hole"
            )
            return base
        mech = (row.get("acceptance_mechanism") or "").strip()
        if not mech:
            mech = acceptance_mechanism(mut_path)
        base["acceptance_mechanism"] = mech
        if mech == "runtime_trap":
            base["review_bucket"] = "discarded-valid"
            base["validity_reason"] += "; violation routed to runtime trap only"
            return base
        if kind not in CHECK_TIME_KINDS or not base["rule_quote"]:
            base["review_bucket"] = "discarded-valid"
            base["validity_reason"] += "; missing check-time spec quote"
            return base
        sub = (row.get("hole_subtype") or "").strip()
        pnote = (row.get("probe_note") or "").strip()
        if not sub:
            if kind == "drop_requires":
                removed = removed_requires_clause(orig, mut) or ""
                if removed:
                    sub, pnote = probe_drop_requires_subtype(mut_path, orig_path, removed)
            elif kind == "widen_requires":
                sub, pnote = "unchecked", "widen probe not implemented"
        base["hole_subtype"] = sub
        base["probe_note"] = pnote
        if sub in UNVERIFIED_SUBTYPES:
            base["review_bucket"] = "discarded-valid"
            base["validity_reason"] += f"; unverified subtype `{sub}`"
            return base
        if mech not in ("check_time_pass", "check_time_violation_unreported"):
            base["review_bucket"] = "discarded-valid"
            base["validity_reason"] += f"; no static no-trap evidence (mech={mech})"
            return base
        base["review_bucket"] = "confirmed-hole"
        return base

    base["review_bucket"] = "discarded-valid"
    return base


def kind_generation_counts(rows: list[dict]) -> dict[str, int]:
    c = Counter(r["mutation_kind"] for r in rows)
    return {k: c.get(k, 0) for k in CANONICAL_KINDS}


def kind_zero_reasons() -> dict[str, str]:
    return {
        "drop_requires": "generated via drop_machine_requires (header)",
        "widen_requires": "generated via widen_requires_lt",
        "remove_guard_transition": "no delete_first_if_guard mutator applied in run 1",
        "remove_mut": "generated",
        "mut_ref_instead_of_shared": "generated via ref_to_mut_ref",
        "shared_instead_of_mut": "generated via mut_ref_to_ref",
        "none": "drop_machine_ensures mapped here; ensures drops not soundness holes",
    }


def prune_mutants(rows: list[dict]) -> tuple[int, int, list[dict]]:
    keep: set[str] = set()
    for r in rows:
        if r["review_bucket"] in ("confirmed-hole", "known-gap-q7-mut"):
            mp = (r.get("mutant_path") or "").strip()
            if mp:
                keep.add((REPO / mp).parent.name)
    kept_dirs = 0
    archived = 0
    to_archive: list[Path] = []
    if MUT_ROOT.is_dir():
        for d in MUT_ROOT.iterdir():
            if not d.is_dir():
                continue
            if d.name in keep:
                kept_dirs += 1
            else:
                to_archive.append(d)
                archived += 1
    if to_archive:
        with tarfile.open(ARCHIVE, "w:gz") as tar:
            for d in to_archive:
                tar.add(d, arcname=d.name)
            for d in to_archive:
                shutil.rmtree(d)
    for r in rows:
        mp = (r.get("mutant_path") or "").strip()
        if not mp:
            continue
        parent_name = (REPO / mp).parent.name
        if parent_name not in keep:
            r["mutant_path"] = ""
            r.setdefault(
                "notes",
                (r.get("notes") or "") + "; mutant archived",
            ).strip("; ")
    return kept_dirs, archived, rows


def hand_check_section(by_kind: dict[str, list[dict]]) -> list[str]:
    lines = [
        "## Hand-check samples (≥10 per kind; confirmed holes or next-best accepted mutants)",
        "",
    ]
    for kind in CANONICAL_KINDS:
        pool = [r for r in by_kind.get(kind, []) if r["review_bucket"] == "confirmed-hole"]
        extra_label = "confirmed holes"
        if len(pool) < 10:
            for bucket in (
                "known-gap-q7-mut",
                "disagreement-valid",
                "discarded-valid",
                "valid-route-outside",
            ):
                for r in sorted(by_kind.get(kind, []), key=lambda x: x["fixture"]):
                    if r["review_bucket"] == bucket and r not in pool:
                        pool.append(r)
                    if len(pool) >= 10:
                        break
                if len(pool) >= 10:
                    break
            extra_label = "confirmed + reviewed accepted mutants"
        if not pool:
            lines.append(f"### `{kind}`")
            lines.append("_No retained mutants to review in this kind._")
            lines.append("")
            continue
        lines.append(f"### `{kind}` ({len(pool)} samples, {extra_label})")
        lines.append("")
        pool.sort(key=lambda r: r["fixture"])
        for i, r in enumerate(pool[:10], 1):
            mp = (REPO / r["mutant_path"]) if r.get("mutant_path") else None
            if mp and mp.is_file():
                diff = minimal_mutant_diff(
                    (REPO / r["fixture"]).read_text(), mp.read_text(), max_lines=12
                )
            else:
                diff = "(mutant archived in shard-2-mutants-pruned.tar.gz)"
            lines.extend(
                [
                    f"#### {i}. `{r['fixture']}`",
                    f"- **accepted_by:** `{r['accepted_by']}` **mech:** `{r.get('acceptance_mechanism', '')}`",
                    f"- **Spec** (`{r['rule_path']}`): {r['rule_quote'][:240]}…"
                    if len(r.get("rule_quote", "")) > 240
                    else f"- **Spec** (`{r['rule_path']}`): {r.get('rule_quote', '')}",
                    f"- **Validity:** {r['validity_reason']}",
                    "",
                    "```diff",
                    diff,
                    "```",
                    "",
                ]
            )
    return lines


def write_tsv(rows: list[dict]) -> None:
    cols = [
        "fixture",
        "tier",
        "mutation_kind",
        "legacy_kind",
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


def main() -> int:
    if not TSV.is_file():
        print("missing", TSV)
        return 1
    if not ROUTE.is_file() or not OMEGA.is_file():
        print("Build omega + route_one_comparison", file=sys.stderr)
        return 1

    q7_idx: dict = {}
    if Q7_INDEX.is_file():
        q7_idx = json.loads(Q7_INDEX.read_text())

    raw = list(csv.DictReader(TSV.open(), delimiter="\t"))
    reviewed = [classify_row(r, q7_idx) for r in raw]
    write_tsv(reviewed)

    genuine_skip, harness_skip = rescan_original_skips()
    prog = json.loads(PROGRESS.read_text()) if PROGRESS.is_file() else {}
    st = prog.setdefault("stats", {})
    st["run2_original_not_accepted"] = genuine_skip
    st["run2_harness_reroute"] = harness_skip
    st["fixtures_skipped_original_fails"] = len(genuine_skip)
    prog["stats"] = st
    PROGRESS.write_text(json.dumps(prog, indent=2))

    bucket_counts = Counter(r["review_bucket"] for r in reviewed)
    kind_counts = kind_generation_counts(reviewed)
    missing_route = sum(
        1 for r in reviewed if not (r.get("route_verdict") or "").strip()
    )
    fixtures_with_rows = {r["fixture"] for r in reviewed}
    fx_total = len(fixture_list())
    no_mut = max(0, fx_total - len(fixtures_with_rows) - len(genuine_skip))
    failed_run_fx = len({r["fixture"] for r in reviewed if r["review_bucket"] == "failed-run"})

    vro = [r for r in reviewed if r["review_bucket"] == "valid-route-outside"]
    vro_break = Counter(r["route_verdict"] for r in vro)

    kept, archived, reviewed = prune_mutants(reviewed)
    write_tsv(reviewed)

    confirmed = [r for r in reviewed if r["review_bucket"] == "confirmed-hole"]
    by_kind: dict[str, list[dict]] = defaultdict(list)
    for r in reviewed:
        by_kind[r["mutation_kind"]].append(r)

    lines = [
        "# Soundness mutation shard 2 (run 2)",
        "",
        f"PIN `{PIN}` | branch `work/soundness-mutations-shard-2`",
        "",
        "## Bucket totals",
        "",
        "| Bucket | Count |",
        "| --- | ---: |",
    ]
    for b in BUCKETS:
        lines.append(f"| `{b}` | {bucket_counts.get(b, 0)} |")
    lines.append(f"| **TSV rows** | **{len(reviewed)}** |")
    lines.append("")

    lines.extend(["## Mutation kind totals (canonical)", ""])
    for k in CANONICAL_KINDS:
        lines.append(
            f"- `{k}`: **{kind_counts[k]}** — {kind_zero_reasons().get(k, '')}"
        )
    lines.append(f"- per-kind sum: **{sum(kind_counts.values())}** (must equal TSV rows)")
    lines.append("")

    lines.extend(
        [
            "## Integrity",
            "",
            f"- Bucket sum equals TSV rows: **{sum(bucket_counts.values()) == len(reviewed)}**",
            f"- Per-kind sum equals TSV rows: **{sum(kind_counts.values()) == len(reviewed)}**",
            f"- Missing route verdicts: **{missing_route}**",
            f"- Confirmed holes with empty spec quote: **{sum(1 for r in confirmed if not (r.get('rule_quote') or '').strip())}**",
            f"- Mutant dirs kept: {kept}; archived: {archived} (`{ARCHIVE.relative_to(REPO)}`)",
            "",
            "## Fixture identity",
            "",
            f"`fixtures ({fx_total}) = mutated ({len(fixtures_with_rows)}) + no-mutation ({no_mut}) + original-not-accepted ({len(genuine_skip)}) + failed-run fixtures ({failed_run_fx})`",
            "",
            f"Check: {len(fixtures_with_rows) + no_mut + len(genuine_skip) + failed_run_fx} "
            f"(expect {fx_total}; failed-run fixture rows may overlap mutated)",
            "",
        ]
    )

    lines.extend(["## valid-route-outside breakdown", ""])
    for rv, n in vro_break.most_common():
        lines.append(f"- `route={rv}`: **{n}**")
    lines.append(f"- total: **{len(vro)}**")
    lines.append("")

    if genuine_skip:
        lines.extend(["## Original-not-accepted (genuine)", ""])
        for rec in genuine_skip[:60]:
            lines.append(
                f"- `{rec['fixture']}`: current={rec['current_verdict']} — {rec['diagnostic'][:160]}"
            )
        if len(genuine_skip) > 60:
            lines.append(f"- … {len(genuine_skip) - 60} more")
        lines.append("")

    if harness_skip:
        lines.extend(["## Harness/environment (not counted as original-not-accepted)", ""])
        for rec in harness_skip:
            lines.append(
                f"- `{rec['fixture']}`: {rec.get('note', rec['diagnostic'][:160])}"
            )
        lines.append("")

    lines.extend(hand_check_section(by_kind))
    MD.write_text("\n".join(lines))

    print("buckets", dict(bucket_counts))
    print("confirmed", len(confirmed))
    print("known-gap-q7-mut", bucket_counts.get("known-gap-q7-mut", 0))
    print("pruned", archived, "kept", kept)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
