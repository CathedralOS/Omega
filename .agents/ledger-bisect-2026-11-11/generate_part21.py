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

WITNESS_MAP = {
    "c0": "pass/facts/non_owning_reference_erasure_through_other_machine_exit",
    "c1": "pass/borrows/dynamic_window_return_custody_exit",
    "c2": "pass/facts/recoverable_replacement_window_through_target_body_exit",
    "c3": "pass/facts/non_owning_reference_erasure_through_target_body_exit",
    "c4": "pass/terminal_psi/debug_identity_complete_module",
    "c5": "pass/control_flow/runtime_ranked_state_cycle_borrowed_record_exit",
}


def parse_summary_line(text: str) -> dict:
    head = text.split("\n", 1)[0]
    if "status=flaky" in head:
        m = re.search(
            r"good=([0-9a-f]{40}) parent=([0-9a-f]{40}) break=([0-9a-f]{40}).*"
            r"verify_break_codes=(\[[^\]]*\]) verify_parent_codes=(\[[^\]]*\])",
            head,
        )
        if m:
            return {
                "status": "flaky",
                "good": m.group(1),
                "parent": m.group(2),
                "break": m.group(3),
                "break_codes": m.group(4),
                "parent_codes": m.group(5),
            }
    if "verdict=never-good" in head or "status=never_good" in head:
        m = re.search(r"intro=([0-9a-f]{40})", head)
        return {"status": "never_good", "intro": m.group(1) if m else ""}
    if "status=propagated" in head:
        m = re.search(r"reason=(\S+)", head)
        return {"status": "propagated", "reason": m.group(1) if m else ""}
    m = re.search(
        r"good=([0-9a-f]{40}) parent=([0-9a-f]{40}) break=([0-9a-f]{40}) pr=(\d*) "
        r"on_first_parent=(\w+) verify_break_codes=(\[[^\]]*\]) verify_parent_codes=(\[[^\]]*\]) status=bisected",
        head,
    )
    if m:
        return {
            "status": "bisected",
            "good": m.group(1),
            "parent": m.group(2),
            "break": m.group(3),
            "pr": m.group(4),
            "on_first_parent": m.group(5),
            "break_codes": m.group(6),
            "parent_codes": m.group(7),
        }
    return {"status": "pending"}


def main() -> None:
    fixtures = [l.strip() for l in (AG / "part21_fixtures.txt").read_text().splitlines() if l.strip()]
    cls = classify_all(AG / "part21_observed.txt", AG / "part21_fixtures.txt")
    obs = parse_records((AG / "part21_observed.txt").read_text())
    clusters = defaultdict(list)
    for fix in cls["broken"]:
        first = (obs[fix]["diagnostics"] or [""])[0]
        clusters[norm_diag(first)].append((fix, first))

    ranked = sorted(clusters.items(), key=lambda kv: (-len(kv[1]), kv[0]))

    lines = [
        "# Ledger bisect part 21 (k=21, N=40)",
        "",
        f"Measured commit (PIN): `{PIN}`",
        "",
        "## Summary",
        "",
        f"- Slice fixtures: **{len(fixtures)}** (must equal fixed + broken below)",
        f"- Fixed on PIN: **{cls['counts']['fixed']}**",
        f"- Still broken on PIN: **{cls['counts']['broken']}**",
        f"- **{cls['counts']['fixed'] + cls['counts']['broken']}** = {len(fixtures)}",
        f"- Diagnostic clusters: **{len(ranked)}**",
        "",
        "## Cluster verdicts",
        "",
    ]
    tsv = ["fixture\tclass\tfirst_diagnostic\tcluster_id\tattribution"]

    for i, (dk, members) in enumerate(ranked):
        cid = f"c{i}"
        fixes = sorted(m[0] for m in members)
        witness = WITNESS_MAP.get(cid, fixes[0])
        log_p = AG / f"bisect-{cid}.log"
        detail_p = AG / f"cluster-{cid}.md"
        bi = parse_summary_line(log_p.read_text()) if log_p.exists() else {"status": "pending"}

        lines.append(f"### {cid} (witness `{witness}`)")
        lines.append("")
        if detail_p.exists():
            lines.append(detail_p.read_text().strip())
            lines.append("")
        elif bi["status"] == "pending":
            lines.append("*Pending bisect.*")
            lines.append("")
        elif bi["status"] == "never_good":
            lines.append(f"Verdict: **never-good since `{bi.get('intro','')}`**")
            lines.append("")
        elif bi["status"] == "flaky":
            lines.append(
                f"Verdict: **flaky** (break codes {bi.get('break_codes')}, parent codes {bi.get('parent_codes')})"
            )
            lines.append("")
        elif bi["status"] == "bisected":
            lines.append(
                f"- Good: `{bi['good']}` | Parent: `{bi['parent']}` | Break: `{bi['break']}` | PR #{bi.get('pr','')}"
            )
            lines.append(f"- First-parent between good and PIN: `{bi.get('on_first_parent')}`")
            lines.append("")

        split_logs = sorted(AG.glob(f"bisect-{cid}.*.log"))
        for sp in split_logs:
            lines.append(f"#### Split `{sp.name}`")
            lines.append("")
            cp = AG / f"cluster-{sp.stem.replace('bisect-', '')}.md"
            if cp.exists():
                lines.append(cp.read_text().strip())
                lines.append("")

        def attr_for(fix: str) -> str:
            short = fix.split("/")[-1]
            sp_log = AG / f"bisect-{cid}.{short}.log"
            if sp_log.exists():
                sb = parse_summary_line(sp_log.read_text())
                if sb["status"] == "bisected":
                    return sb["break"]
            if bi["status"] == "bisected":
                flip_md = AG / f"cluster-{cid}.md"
                if flip_md.exists() and fix in flip_md.read_text():
                    for line in flip_md.read_text().splitlines():
                        if f"`{fix}`" in line and "|" in line:
                            parts = [p.strip() for p in line.split("|")]
                            if len(parts) >= 4:
                                pe, be = parts[2], parts[3]
                                if pe == "0" and be == "1":
                                    return bi["break"]
            if bi["status"] == "bisected":
                return bi["break"]
            return bi["status"]

        for f in fixes:
            attr = attr_for(f)
            tsv.append(f"{f}\tstill broken\t{(obs[f]['diagnostics'] or [''])[0][:200]}\t{cid}\t{attr}")
            lines.append(f"- `{f}` → attribution: `{attr}`")

        lines.append("")

    lines += ["## Fixed on PIN", ""]
    for f in sorted(cls["fixed"]):
        lines.append(f"- `{f}`")
        tsv.append(f"{f}\tfixed\t\tfixed\tfixed")

    lines += [
        "",
        "## Method",
        "",
        "- Fixed PIN ledger + PIN witness observation in `/tmp`.",
        "- Good: exponential first-parent walk back from PIN; refine to **latest** exit 0.",
        "- Break: `git bisect --first-parent` between good and PIN; per-commit `target/verify-<sha>`.",
        "- Confirm: 3× exit 1 at break, 3× exit 0 at break^1 (PIN diagnostic match); else **flaky**.",
        "",
    ]
    (AG / "part-21.md").write_text("\n".join(lines) + "\n")
    (AG / "part-21.tsv").write_text("\n".join(tsv) + "\n")


if __name__ == "__main__":
    main()
