#!/usr/bin/env python3
"""Render part-29.md and part-29.tsv from measurement + bisect JSON files."""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path("/agent/repos/omega")
OUT = ROOT / ".agents/ledger-bisect-2026-11-11"
PIN = "51e8c45c793007f211d0d9baee7de16767172444"

CLUSTERS = [
    {
        "id": "c0",
        "rows": 1,
        "kind": "pass/run rejected",
        "diagnostic": "`two_children` has no executable body: this compiler does not lower a statement of it yet",
        "witness": "pass/ownership/runtime_match_conditional_observation_cleanup_exit",
        "members": ["pass/ownership/runtime_match_conditional_observation_cleanup_exit"],
    },
    {
        "id": "c1",
        "rows": 1,
        "kind": "incomplete",
        "diagnostic": "`main` has no executable body: call operation: structural result loan",
        "witness": "run/borrows/returned_descriptor_entry_snapshot",
        "members": ["run/borrows/returned_descriptor_entry_snapshot"],
    },
    {
        "id": "c2",
        "rows": 1,
        "kind": "incomplete",
        "diagnostic": "`main` has no executable body: reference result has no complete captured origin paths",
        "witness": "run/slices/returned_record_fixed_array_window",
        "members": ["run/slices/returned_record_fixed_array_window"],
    },
    {
        "id": "c3",
        "rows": 1,
        "kind": "pass/run rejected",
        "diagnostic": "cannot prove requires contract for call capacity from geometry",
        "witness": "pass/proofs/reviewed_mathematical_data_contract_exit",
        "members": ["pass/proofs/reviewed_mathematical_data_contract_exit"],
    },
]

FIXED = [
    "pass/layouts/placed_reference_results/two_live",
    "pass/recast/returned_qualification_preserves_representation_exit",
    "pass/recast/returned_qualified_reference_exit",
    "pass/borrows/runtime_stored_carrier_exchange_exit",
    "fail/calls/static_boundary_statement_unrelated_evidence",
    "fail/traits/explicit_implementation_unrelated_trait",
    "fail/layouts/placed_stable_array_readonly_write",
    "fail/traits/prepared_callable_wrong_member_source",
]

TSV_ROWS = []
for fix in FIXED:
    TSV_ROWS.append((fix, "fixed_on_main", "", ""))
for c in CLUSTERS:
    for m in c["members"]:
        TSV_ROWS.append((m, "still_broken", c["diagnostic"][:120], c["id"]))


def main() -> None:
    lines = [
        "# Ledger bisect part 29 (k=29, N=40)",
        "",
        f"Measured PIN: `{PIN}`",
        "",
        "## Summary",
        "",
        f"- Slice rows (prior report clusters index % 40 == 29): 11",
        f"- Fixed on PIN: {len(FIXED)}",
        f"- Still broken (re-clustered): 4",
        "",
        "## Clusters",
        "",
        "| Rows | Kind | Diagnostic | Witness | Break | PR | Mechanism | Status |",
        "| ---: | --- | --- | --- | --- | ---: | --- | --- |",
    ]
    for c in CLUSTERS:
        jpath = OUT / f"{c['id']}.json"
        if jpath.exists():
            j = json.loads(jpath.read_text())
            br = j.get("break", "")[:12]
            pr = j.get("pr", "")
            mech = (j.get("mechanism") or "")[:100]
            st = j.get("status", "pending")
        else:
            br = pr = mech = st = "pending"
        lines.append(
            f"| {c['rows']} | {c['kind']} | {c['diagnostic'][:60]} | `{c['witness']}` | `{br}` | {pr} | {mech} | {st} |"
        )
    lines.extend(["", "## Members", ""])
    for c in CLUSTERS:
        jpath = OUT / f"{c['id']}.json"
        lines.append(f"### {c['id']}")
        for m in c["members"]:
            lines.append(f"- `{m}` still broken")
        if jpath.exists():
            j = json.loads(jpath.read_text())
            lines.append(f"- good first-parent: `{j.get('good','')[:12]}`")
    lines.extend(["", "## Fixed on main", ""])
    for f in FIXED:
        lines.append(f"- `{f}`")
    lines.extend(
        [
            "",
            "## Commands",
            "",
            "- `git fetch --unshallow origin`",
            "- `python3 .agents/ledger-bisect-tools/pin_bisect_state.py <witnesses...>`",
            "- `bash .agents/ledger-bisect-tools/bisect_cluster.sh <id> <witness> <out.json>`",
            "- Host: linux x86_64; `OMEGA_CORPUS_JOBS=1`; direct `corpus_runner`",
        ]
    )
    (OUT / "part-29.md").write_text("\n".join(lines) + "\n")
    tsv = ["fixture\tclass\tfirst_diagnostic\tcluster_id"]
    tsv.extend("\t".join(row) for row in TSV_ROWS)
    (OUT / "part-29.tsv").write_text("\n".join(tsv) + "\n")


if __name__ == "__main__":
    main()
