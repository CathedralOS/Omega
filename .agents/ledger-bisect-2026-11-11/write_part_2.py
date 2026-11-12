#!/usr/bin/env python3
"""Write part-2.md and part-2.tsv from PIN analysis + bisect results."""
from __future__ import annotations

import json
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DIR = REPO / ".agents/ledger-bisect-2026-11-11"
PIN = "51e8c45c793007f211d0d9baee7de16767172444"

CLUSTER_WITNESS = [
    ("cannot prove published termination", "run/ownership/sequence_owned_local_cleanup"),
    ("data `", "pass/facts/static_callback_reach_through_field"),
    ("partial write enclosure has no exact active-case", "pass/borrows/returned_record_case_field_store_exit"),
    ("source parent type is unavailable", "pass/borrows/runtime_reassigned_view_capture_exit"),
    ("selected optimizations", "pass/optimizer/pre_allocation_joint_frontier_exit"),
]


def load_bisect(witness: str) -> dict:
    p = DIR / "bisect_results" / f"{witness.replace('/', '_')}.json"
    return json.loads(p.read_text()) if p.exists() else {"status": "pending"}


def main() -> int:
    analysis = json.loads((DIR / "slice_2_pin_analysis.json").read_text())
    clusters = analysis["clusters"]
    md = [
        "# Ledger bisect part 2",
        "",
        f"PIN `{PIN}`. Method: PIN-fixed `corpus_outcomes.txt` + witness observation; "
        "bisect test never reads ledger at checkout; ledger-only commits skipped (exit 125).",
        "",
        f"**Re-measure:** {len(analysis['fixed']) + len(analysis['broken'])} prior-slice fixtures; "
        f"**fixed on main** {len(analysis['fixed'])}; **still broken** {len(analysis['broken'])}; "
        f"**PIN clusters** {len(clusters)}.",
        "",
        "| rows | diagnostic (PIN) | witness | breaking commit | PR | mechanism | status |",
        "| ---: | --- | --- | --- | ---: | --- | --- |",
    ]
    tsv: list[str] = []
    for i, c in enumerate(clusters):
        w = c["witness"]
        for sub, bw in CLUSTER_WITNESS:
            if sub in c["diag"]:
                w = bw
                break
        b = load_bisect(w)
        diag = c["diag"].replace("|", "\\|")[:96]
        mech = (b.get("mechanism") or "").replace("|", "\\|")[:90]
        bc = (b.get("breaking_commit") or "")[:12]
        md.append(
            f"| {c['count']} | {diag} | `{w}` | `{bc}` | {b.get('pr') or ''} | {mech} | {b.get('status', 'pending')} |"
        )
        for m in c["members"]:
            md.append(f"- `{m}`")
            tsv.append(f"{m}\tstill broken\t{c['diag'][:200]}\t2-pin-{i}")
        md.append("")
    md += ["## Fixed on main", ""]
    for f in analysis["fixed"]:
        md.append(f"- `{f}`")
        tsv.append(f"{f}\tfixed on main\t\t")
    md += [
        "",
        "## Commands",
        "",
        "- Members: `recover_slice_members.py 2 40`",
        "- Measure: direct `corpus_runner`, `OMEGA_CORPUS_JOBS=1`",
        "- Bisect: `pin_snapshot.py` + `bisect_one.sh` (pin-fixed ledger)",
        "- Host: linux x86_64",
    ]
    (DIR / "part-2.md").write_text("\n".join(md) + "\n")
    (DIR / "part-2.tsv").write_text("\n".join(tsv) + "\n")
    print("wrote part-2.md")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
