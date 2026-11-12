#!/usr/bin/env python3
"""Rewrite part-27.md and part-27.tsv from fresh PIN records and cluster results."""
from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path("/agent/repos/omega")
OUT = ROOT / ".agents/ledger-bisect-2026-11-11"
STATE = Path("/tmp/ledger-bisect-part27")


def load_results() -> list[dict]:
    rows = []
    for path in sorted((STATE / "results").glob("*.json")):
        rows.append(json.loads(path.read_text()))
    return rows


def main() -> int:
    measured = json.loads((STATE / "measured.json").read_text())
    results = {r["cluster"]: r for r in load_results()}
    fixed = [m for m in measured if m["cls"] == "fixed"]
    broken = [m for m in measured if m["cls"] == "broken"]
    lines = []
    lines.append("# Ledger bisect part 27 (k=27, N=40)")
    lines.append("")
    lines.append("Measured commit (PIN): `51e8c45c793007f211d0d9baee7de16767172444`.")
    lines.append("")
    lines.append(
        "Re-measured with a commit-scoped target directory `/tmp/bisect-target-<sha>`. "
        "Bisect oracle: exit 0 matches the fixed PIN ledger expectation, exit 1 is the PIN "
        "status plus the PIN diagnostic class, exit 125 is any other outcome, a build failure, "
        "or a ledger-only commit. Good commits come from an exponential first-parent walk "
        "(1, 2, 4, 8, …) with no binary search. `git bisect start --first-parent` runs between "
        "that good commit and the PIN."
    )
    lines.append("")
    lines.append("## Counts")
    lines.append("")
    lines.append("| | Count |")
    lines.append("| --- | ---: |")
    lines.append(f"| Rows re-measured | {len(measured)} |")
    lines.append(f"| Fixed on PIN | {len(fixed)} |")
    lines.append(f"| Still broken on PIN | {len(broken)} |")
    lines.append(f"| Clusters with a finished result | {len(results)} |")
    lines.append("")
    lines.append("## Clusters")
    lines.append("")
    lines.append(
        "| id | rows | witness | good | breaking | PR | parent 3x | bad 3x | status |"
    )
    lines.append("| --- | ---: | --- | --- | --- | --- | --- | --- | --- |")
    for cid in [f"c0{i}" for i in range(1, 8)]:
        r = results.get(cid)
        if not r:
            lines.append(f"| {cid} |  |  |  |  |  |  |  | pending |")
            continue
        lines.append(
            "| {id} | {n} | `{w}` | `{good}` | `{bad}` | {pr} | {p3} | {b3} | {st} |".format(
                id=cid,
                n=len(r.get("members") or []),
                w=r.get("witness", ""),
                good=r.get("good") or "",
                bad=r.get("breaking") or "",
                pr=r.get("pr") or "",
                p3=",".join(str(x) for x in r.get("parent_runs") or []),
                b3=",".join(str(x) for x in r.get("bad_runs") or []),
                st=r.get("status") or "",
            )
        )
    lines.append("")
    for cid in [f"c0{i}" for i in range(1, 8)]:
        r = results.get(cid)
        if not r:
            continue
        lines.append(f"### {cid}")
        lines.append("")
        lines.append(f"Witness: `{r.get('witness','')}`")
        lines.append("")
        lines.append("Members: " + " ".join(f"`{m}`" for m in r.get("members") or []))
        lines.append("")
        if r.get("stop_matching") and r.get("stop_matching") != r.get("breaking"):
            lines.append(
                f"First commit where the row stops matching the ledger: `{r['stop_matching']}`."
            )
            lines.append("")
            lines.append(
                f"First commit showing the current diagnostic class: `{r.get('breaking','')}`."
            )
            lines.append("")
        if r.get("note"):
            lines.append(r["note"])
            lines.append("")
        lines.append("Mechanism:")
        lines.append("")
        lines.append("```")
        lines.append(r.get("mechanism") or "mechanism not established")
        lines.append("```")
        lines.append("")
        runs = r.get("member_runs") or {}
        if runs:
            lines.append("Member checks at the parent and at the breaking commit:")
            lines.append("")
            for name, info in runs.items():
                lines.append(
                    f"- `{name}` parent={info.get('at_parent')} breaking={info.get('at_bad')}"
                )
            lines.append("")
    lines.append("## Fixed on PIN")
    lines.append("")
    for m in fixed:
        lines.append(f"- `{m['fixture']}`")
    lines.append("")
    lines.append("## Still broken on PIN")
    lines.append("")
    for m in broken:
        lines.append(f"- `{m['fixture']}` — {m.get('diag','')}")
    lines.append("")
    lines.append("## Commands")
    lines.append("")
    lines.append("```bash")
    lines.append("cd /agent/repos/omega")
    lines.append("# per commit: CARGO_TARGET_DIR=/tmp/bisect-target-<40-char-sha>")
    lines.append("bash /tmp/ledger-bisect-part27/tools/bisect_witness.sh")
    lines.append("```")
    lines.append("")
    lines.append("Host: cloud VM. One `mbx` build at a time. `OMEGA_CORPUS_JOBS=1`.")
    lines.append("")
    (OUT / "part-27.md").write_text("\n".join(lines))
    tsv = ["fixture\tclass\tfirst_diagnostic\tcluster_id"]
    for m in measured:
        diag = (m.get("diag") or "").replace("\t", " ")
        tsv.append(f"{m['fixture']}\t{m['cls']}\t{diag}\t{m.get('cluster') or ''}")
    (OUT / "part-27.tsv").write_text("\n".join(tsv) + "\n")
    print(f"wrote {len(results)} cluster results")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
