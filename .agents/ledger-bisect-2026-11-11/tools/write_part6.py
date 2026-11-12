#!/usr/bin/env python3
"""Write part-6.md and part-6.tsv from classification + bisect JSON files."""
from __future__ import annotations

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PART_DIR = ROOT / ".agents/ledger-bisect-2026-11-11"
PART = 6


def provenance() -> str:
    try:
        return subprocess.check_output(
            ["cargo", "xtask", "provenance"], cwd=ROOT, text=True, stderr=subprocess.DEVNULL
        ).strip()
    except Exception:
        return "Agent: Cursor | Model: unknown | Effort: unknown | Tokens: unknown | Steps: unknown | Time: unknown | Steers: 0"


def main() -> int:
    clf = json.loads((PART_DIR / "slice6_classification.json").read_text(encoding="utf-8"))
    bisects: dict[int, dict] = {}
    for p in sorted(PART_DIR.glob("bisect_cluster*.json")):
        idx = int(p.stem.replace("bisect_cluster", ""))
        bisects[idx] = json.loads(p.read_text(encoding="utf-8"))

    lines = [
        f"# Ledger bisect part {PART}",
        "",
        f"Measured commit (PIN): `{clf['pin']}`",
        "",
        "## Summary",
        "",
        f"- Prior-report slice indices (index % 40 == {PART}): "
        + ", ".join(str(a["old_index"]) for a in clf["old_assign"]),
        f"- Rows re-measured: {len(clf['fixed']) + len(clf['broken'])} "
        f"(fixed on main: {len(clf['fixed'])}, still broken: {len(clf['broken'])})",
        f"- PIN clusters (by first diagnostic): {len(clf['clusters'])} (bisected: {len(bisects)})",
        "",
        "## PIN clusters",
        "",
        "| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |",
        "| ---: | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for c in sorted(clf["clusters"], key=lambda x: (-x["count"], x["diag_sample"])):
        b = bisects.get(c["cluster_id"], {})
        if b.get("breaking_commit") in ("", "51e8c45c793007f211d0d9baee7de16767172444") and b.get("status") == "bisected":
            b = {**b, "status": "propagated", "stability": (b.get("stability") or "") + ";invalid_break"}
        diag = c["diag_sample"].replace("|", "\\|")[:100]
        mech = (b.get("mechanism") or "").replace("|", "\\|")[:120]
        brk = (b.get("breaking_commit") or "")[:12]
        pr = b.get("pr") or ""
        st = b.get("status", "pending")
        if b.get("stability"):
            st += f" ({b['stability']})"
        lines.append(
            f"| {c['count']} | {c['kind']} | {diag} | `{c['witness']}` | `{brk}` | {pr} | {mech} | {st} |"
        )
        mline = ", ".join(f"`{x}`" for x in c["members"])
        lines.append(f"Members: {mline}")
        lines.append("")

    lines += ["## Fixed on main (slice rows)", ""]
    for f in clf["fixed"]:
        lines.append(f"- `{f}`")
    lines.append("")

    lines += ["## Prior-report assignment (member recovery)", ""]
    for a in clf["old_assign"]:
        lines.append(
            f"- old index {a['old_index']}: {a['rows']} rows, witness `{a['witness']}`, "
            f"members measured {len(a['members'])}"
        )
    lines.append("")

    lines += [
        "## Commands",
        "",
        "- Host: linux x86_64, `OMEGA_CORPUS_JOBS=1`",
        "- `bash .agents/ledger-bisect-2026-11-11/tools/measure_pin.sh slice6_fixtures.txt observed_pin.txt`",
        "- `git bisect start <PIN> <good> --first-parent` + `git bisect run .agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh`",
        "",
        provenance(),
    ]

    out_md = PART_DIR / f"part-{PART}.md"
    out_md.write_text("\n".join(lines) + "\n", encoding="utf-8")

    tsv_lines = ["fixture\tclass\tfirst_diagnostic\tcluster_id"]
    for c in clf["clusters"]:
        cid = c["cluster_id"]
        for f in c["members"]:
            row = next((x for x in clf["broken"] if x["fixture"] == f), None)
            if row:
                tsv_lines.append(
                    f"{f}\tstill broken\t{row['diag'].replace(chr(9),' ')}\t{cid}"
                )
    for f in clf["fixed"]:
        tsv_lines.append(f"{f}\tfixed on main\t\t")
    (PART_DIR / f"part-{PART}.tsv").write_text("\n".join(tsv_lines) + "\n", encoding="utf-8")
    print(out_md)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
