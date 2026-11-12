#!/usr/bin/env python3
"""Parse prior report clusters and return slice k=11 members for part bisect."""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
K = int(sys.argv[1]) if len(sys.argv) > 1 else 11
N = int(sys.argv[2]) if len(sys.argv) > 2 else 40


def fetch_report() -> str:
    return subprocess.check_output(
        [
            "git",
            "show",
            "origin/cursor/ledger-regressions-report-d9c6:wiki/drafts/audits/ledger_regressions_2026-11-11.md",
        ],
        cwd=REPO,
        text=True,
    )


def parse_clusters(text: str) -> list[dict]:
    section = text.split("## Clusters still broken", 1)[1]
    section = section.split("\n## ", 1)[0]
    clusters = []
    for block in re.split(r"\n(?=\| \d+ \|)", section):
        m = re.match(
            r"\| (\d+) \| ([^|]+) \| ([^|]+) \| `([^`]+)` \| `([^`]+)` \|",
            block.strip(),
        )
        if not m:
            continue
        rows, kind, diag, witness, breaking = [x.strip() for x in m.groups()]
        mem_m = re.search(r"Members: (.+?)(?:\n\n|\Z)", block, re.DOTALL)
        members = re.findall(r"`([^`]+)`", mem_m.group(1)) if mem_m else []
        more_m = re.search(r"\(\+(\d+) more\)", mem_m.group(1) if mem_m else "")
        clusters.append(
            {
                "rows": int(rows),
                "kind": kind,
                "diag": diag,
                "witness": witness,
                "breaking_propagated": breaking.strip(),
                "members_listed": members,
                "members_more": int(more_m.group(1)) if more_m else 0,
            }
        )
    clusters.sort(key=lambda c: (-c["rows"], c["diag"]))
    return clusters


def expand_cluster_11_members(listed: list[str], rows: int) -> list[str]:
    """Recover members for the 14-row fail selected-map cluster from regressed metadata."""
    regressed = (
        subprocess.check_output(
            [
                "git",
                "show",
                "origin/cursor/ledger-regressions-report-d9c6:wiki/drafts/audits/regressed_fixtures.txt",
            ],
            cwd=REPO,
            text=True,
        )
        .strip()
        .splitlines()
    )
    regressed_set = set(regressed)
    # Known cluster fingerprints from report header + sweep witness family.
    needles = (
        "evidence_application_mismatch",
        "evidence_domain_shadow",
        "evidence_equation_shadow",
        "evidence_formal_shadow",
        "evidence_name_shadow",
        "callback_contract_mismatch",
        "callback_expression_shadow",
        "callback_formal_shadow",
        "callback_result_shadow",
        "callback_static_argument_shadow",
        "value_carrier_mismatch",
        "value_index_invalid",
        "value_type_shadow",
        "static_callback_reach_captured_shadow",
        "cross_module_const_alias_shadowed_evidence",
    )
    expanded = set(listed)
    for fix in regressed:
        if not fix.startswith("fail/"):
            continue
        if any(n in fix for n in needles):
            expanded.add(fix)
    expanded = sorted(x for x in expanded if x in regressed_set)
    if len(expanded) < rows:
        # Fall back to listed only if heuristic under-fills; caller records propagation.
        return sorted(set(listed))
    return expanded[:rows] if len(expanded) > rows else expanded


def main() -> int:
    clusters = parse_clusters(fetch_report())
    slice_entries = []
    for idx in range(len(clusters)):
        if idx % N != K:
            continue
        c = clusters[idx].copy()
        c["prior_cluster_index"] = idx
        members = c["members_listed"]
        if c["members_more"] and idx == 11:
            members = expand_cluster_11_members(members, c["rows"])
        c["members"] = members
        slice_entries.append(c)
    out = REPO / f".agents/ledger-bisect-2026-11-11/slice{K}_from_report.json"
    out.write_text(json.dumps(slice_entries, indent=2))
    all_m = sorted({m for e in slice_entries for m in e["members"]})
    (REPO / f".agents/ledger-bisect-2026-11-11/slice{K}_fixtures.txt").write_text(
        "\n".join(all_m) + "\n"
    )
    print(json.dumps({"clusters": len(slice_entries), "fixtures": len(all_m)}))
    for e in slice_entries:
        print(
            f"idx={e['prior_cluster_index']} rows={e['rows']} members={len(e['members'])} witness={e['witness']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
