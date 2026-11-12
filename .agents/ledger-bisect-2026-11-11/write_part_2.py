#!/usr/bin/env python3
"""Write part-2.md and part-2.tsv from PIN analysis + bisect results."""
from __future__ import annotations

import json
from pathlib import Path

DIR = Path(__file__).resolve().parent
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


def fmt_codes(codes: list[int] | None) -> str:
    if not codes:
        return "—"
    return ",".join(str(c) for c in codes)


def main() -> int:
    analysis = json.loads((DIR / "slice_2_pin_analysis.json").read_text())
    clusters = analysis["clusters"]
    md = [
        "# Ledger bisect part 2",
        "",
        f"PIN `{PIN}`. PIN-fixed ledger + observation; ledger-only commits skipped in `git bisect run`.",
        "",
        f"**Re-measure:** {len(analysis['fixed']) + len(analysis['broken'])} fixtures; "
        f"**fixed on main** {len(analysis['fixed'])}; **still broken** {len(analysis['broken'])}; "
        f"**clusters** {len(clusters)}.",
        "",
    ]
    tsv: list[str] = []
    for i, c in enumerate(clusters):
        w = c["witness"]
        for sub, bw in CLUSTER_WITNESS:
            if sub in c["diag"]:
                w = bw
                break
        b = load_bisect(w)
        diag = c["diag"].replace("|", "\\|")[:120]
        status = b.get("status", "pending")
        mech = (b.get("mechanism") or "").strip() or "mechanism not established"
        md.append(f"## Cluster {i} ({c['count']} rows)")
        md.append("")
        md.append(f"- **Diagnostic:** {diag}")
        md.append(f"- **Witness:** `{w}`")
        md.append(f"- **Status:** {status}")
        if status == "never_good_since_fixture_add":
            md.append(f"- **Verdict:** {b.get('verdict', '')}")
        elif status == "bisected":
            md.append(f"- **First bad:** `{b.get('breaking_commit', '')}`")
            md.append(f"- **PR:** {b.get('pr') or '—'}")
            md.append(f"- **Mechanism:** {mech}")
            md.append(f"- **Exit codes @ first-bad (3×):** {fmt_codes(b.get('exit_codes_first_bad'))}")
            md.append(f"- **Exit codes @ first parent (3×):** {fmt_codes(b.get('exit_codes_first_parent'))}")
        md.append(f"- **Members:** {', '.join(f'`{m}`' for m in c['members'])}")
        if c["count"] >= 2 and b.get("membership_pin_check"):
            md.append("- **Membership @ PIN (3× each, pin-fixed):**")
            for m, rcs in b["membership_pin_check"].items():
                md.append(f"  - `{m}`: {fmt_codes(rcs)}")
        md.append("")
        for m in c["members"]:
            tsv.append(f"{m}\tstill broken\t{c['diag'][:200]}\t2-pin-{i}")
    md += ["## Fixed on main", ""]
    for f in analysis["fixed"]:
        md.append(f"- `{f}`")
        tsv.append(f"{f}\tfixed on main\t\t")
    (DIR / "part-2.md").write_text("\n".join(md) + "\n")
    (DIR / "part-2.tsv").write_text("\n".join(tsv) + "\n")
    print("wrote part-2.md")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
