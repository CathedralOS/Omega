#!/usr/bin/env python3
"""Refresh part-29.md/tsv from JSON bisect outputs with one-line mechanisms."""
from __future__ import annotations

import json
from pathlib import Path

OUT = Path("/agent/repos/omega/.agents/ledger-bisect-2026-11-11")
PIN = "51e8c45c793007f211d0d9baee7de16767172444"

MECHANISM = {
    "c0": "case-pattern common-field preservation stopped lowering two_children observation cleanup (#2078)",
    "c1": "incoming backing / stored-call structural result loans stopped lowering returned descriptor entry (#1794)",
}

CLUSTERS = [
    ("c0", 1, "pass/run rejected", "`two_children` has no executable body", "pass/ownership/runtime_match_conditional_observation_cleanup_exit"),
    ("c1", 1, "incomplete", "`main` has no executable body: structural result loan", "run/borrows/returned_descriptor_entry_snapshot"),
    ("c2", 1, "incomplete", "`main` has no executable body: captured origin paths", "run/slices/returned_record_fixed_array_window"),
    ("c3", 1, "pass/run rejected", "cannot prove requires contract for call capacity from geometry", "pass/proofs/reviewed_mathematical_data_contract_exit"),
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


def load(cid: str) -> dict:
    p = OUT / f"{cid}.json"
    return json.loads(p.read_text()) if p.exists() else {}


def main() -> None:
    broken = 0
    fixed_n = len(FIXED)
    lines = [
        "# Ledger bisect part 29 (k=29, N=40)",
        "",
        f"Measured PIN: `{PIN}`",
        "",
        "## Summary",
        "",
        "- Prior-report slice clusters (index % 40 == 29): 5 original clusters, 11 member rows",
        f"- Fixed on PIN: {fixed_n}",
        "- Still broken at PIN (re-clustered by first diagnostic): 4",
        "",
        "## Clusters",
        "",
        "| Rows | Kind | Diagnostic | Witness | Break | PR | Mechanism | Status |",
        "| ---: | --- | --- | --- | --- | ---: | --- | --- |",
    ]
    tsv = ["fixture\tclass\tfirst_diagnostic\tcluster_id"]
    for f in FIXED:
        tsv.append(f"{f}\tfixed_on_main\t\t")
    for cid, rows, kind, diag, witness in CLUSTERS:
        j = load(cid)
        if j.get("status") != "bisected":
            broken += rows
        br = (j.get("break") or "")[:12]
        pr = j.get("pr") or ""
        mech = MECHANISM.get(cid) or (j.get("mechanism") or "")[:90]
        st = j.get("status", "pending")
        if st == "bisected" and br == PIN[:12]:
            st = "propagated"
            mech = "first_bad_equals_pin"
        lines.append(
            f"| {rows} | {kind} | {diag[:58]} | `{witness}` | `{br}` | {pr} | {mech} | {st} |"
        )
        tsv.append(f"{witness}\tstill_broken\t{diag[:120]}\t{cid}")
    lines.extend(["", "## Members", ""])
    for cid, rows, kind, diag, witness in CLUSTERS:
        j = load(cid)
        lines.append(f"### {cid}")
        lines.append(f"- `{witness}`")
        if j.get("good"):
            lines.append(f"- good (first-parent search): `{j['good'][:12]}`")
    lines.extend(["", "## Fixed on main", ""])
    for f in FIXED:
        lines.append(f"- `{f}`")
    lines.extend(
        [
            "",
            "## Commands",
            "",
            "- `git fetch --unshallow origin`",
            "- `python3 .agents/ledger-bisect-tools/pin_bisect_state.py` (4 witnesses)",
            "- PIN-fixed `/tmp/pin_corpus_outcomes.txt`, `/tmp/bisect_pin_obs.json`",
            "- `bash /tmp/ledger-bisect-tools/bisect_cluster.sh` per cluster",
            "- Host: linux x86_64; `OMEGA_CORPUS_JOBS=1`; direct `corpus_runner`",
        ]
    )
    (OUT / "part-29.md").write_text("\n".join(lines) + "\n")
    (OUT / "part-29.tsv").write_text("\n".join(tsv) + "\n")


if __name__ == "__main__":
    main()
