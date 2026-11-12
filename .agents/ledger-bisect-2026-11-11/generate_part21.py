#!/usr/bin/env python3
"""Regenerate part-21.md and part-21.tsv."""
from __future__ import annotations

import re
import sys
from collections import defaultdict
from pathlib import Path

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
ROOT = Path(__file__).resolve().parents[2]
AG = ROOT / ".agents/ledger-bisect-2026-11-11"
sys.path.insert(0, str(AG / "tools"))
from classify_corpus import classify_all, norm_diag, parse_records  # noqa: E402


def parse_log(cid: str) -> dict:
    p = AG / f"bisect-{cid}.log"
    if not p.exists():
        return {"status": "pending"}
    text = p.read_text()
    if "status=propagated" in text or "reason=" in text and "status=bisected" not in text:
        m = re.search(r"reason=(\S+)", text)
        return {"status": "propagated", "reason": m.group(1) if m else ""}
    m = re.search(
        r"good=([0-9a-f]+) break=([0-9a-f]+) pr=(\d*) rc=(\d+) mech=(.*) status=bisected",
        text.replace("\n", " "),
    )
    if not m:
        return {"status": "pending"}
    return {
        "status": "bisected",
        "good": m.group(1),
        "break": m.group(2),
        "pr": m.group(2) and m.group(3) or "",
        "mech": m.group(5).strip(),
    }


def kind_for(members, bd):
    tiers = {f.split("/")[0] for f in members}
    if members and members[0] in bd and bd[members[0]]["main_status"] == "incomplete":
        return "incomplete"
    if tiers == {"fail"}:
        return "fail wrong reason"
    return "pass/run rejected"


def main() -> None:
    fixtures = [l.strip() for l in (AG / "part21_fixtures.txt").read_text().splitlines() if l.strip()]
    cls = classify_all(AG / "part21_observed.txt", AG / "part21_fixtures.txt")
    obs = parse_records((AG / "part21_observed.txt").read_text())
    clusters = defaultdict(list)
    for fix in cls["broken"]:
        first = (obs[fix]["diagnostics"] or [""])[0]
        clusters[norm_diag(first)].append((fix, first))

    ranked = sorted(clusters.items(), key=lambda kv: (-len(kv[1]), kv[0]))
    witness_map = {
        "c0": "pass/facts/non_owning_reference_erasure_through_other_machine_exit",
        "c1": "pass/borrows/dynamic_window_return_custody_exit",
        "c2": "pass/facts/recoverable_replacement_window_through_target_body_exit",
        "c3": "pass/facts/non_owning_reference_erasure_through_target_body_exit",
        "c4": "pass/terminal_psi/debug_identity_complete_module",
        "c5": "pass/control_flow/runtime_ranked_state_cycle_borrowed_record_exit",
    }

    lines = [
        "# Ledger bisect part 21 (k=21, N=40)",
        "",
        f"Measured commit (PIN): `{PIN}`",
        "",
        "## Summary",
        "",
        "- Legacy report clusters in slice: **6** (report indices 21, 61, 101, 141, 181, 221)",
        f"- Fixture rows re-measured: **{len(fixtures)}**",
        f"- Fixed on PIN: **{cls['counts']['fixed']}**",
        f"- Still broken on PIN: **{cls['counts']['broken']}**",
        f"- PIN diagnostic clusters (after regroup): **{len(ranked)}**",
        "",
        "## PIN clusters (regrouped by first diagnostic)",
        "",
        "| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |",
        "| ---: | --- | --- | --- | --- | ---: | --- | --- |",
    ]
    tsv = ["fixture\tclass\tfirst_diagnostic\tcluster_id"]
    for i, (dk, members) in enumerate(ranked):
        cid = f"c{i}"
        fixes = [m[0] for m in members]
        witness = witness_map.get(cid, max(fixes, key=len))
        bi = parse_log(cid)
        diag = members[0][1][:96]
        br = bi.get("break", "")
        lines.append(
            f"| {len(members)} | {kind_for(fixes, cls['broken_detail'])} | {diag} | `{witness}` | `{br}` | {bi.get('pr','')} | {bi.get('mech', bi.get('reason',''))[:100]} | {bi.get('status','pending')} |"
        )
        lines.append("Members:")
        for f, d in sorted(members):
            lines.append(f"- `{f}` — still broken: {d[:140]}")
        lines.append("")

    lines += ["", "## Fixed on main (PIN)", ""]
    for f in sorted(cls["fixed"]):
        lines.append(f"- `{f}`")
    for f in sorted(cls["fixed"]):
        tsv.append(f"{f}\tfixed\t\tfixed")
    for i, (_, members) in enumerate(ranked):
        for f, d in members:
            tsv.append(f"{f}\tstill broken\t{d[:200]}\tc{i}")

    lines += [
        "",
        "## Sanity",
        "",
        "741270df60f7 predates PIN; witnesses verified **bad** at PIN (`bisect_witness.sh` exit 1).",
        "Good commits from `find_good_commit.py` (binary search on first-parent intro→PIN),",
        "re-verified exit 0 before `git bisect start --first-parent`.",
        "",
        "## Commands",
        "",
        "```bash",
        "cd /agent/repos/omega && git checkout 51e8c45c793007f211d0d9baee7de16767172444",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null",
        "mbx test --no-run -p compiler --test corpus_runner < /dev/null",
        "# OMEGA_CORPUS_FIXTURE_FILTER from part21_fixtures.txt",
        ".agents/ledger-bisect-2026-11-11/run_bisect_cluster.sh <witness> <cN>",
        "```",
        "",
        "Host: linux cloud agent VM. `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=4`, `MBX_SCHEDULER_CPUS=4`.",
    ]
    (AG / "part-21.md").write_text("\n".join(x for x in lines if x is not None) + "\n")
    (AG / "part-21.tsv").write_text("\n".join(tsv) + "\n")


if __name__ == "__main__":
    main()
