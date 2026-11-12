#!/usr/bin/env python3
"""Write part-7.md and part-7.tsv from measurement artifacts."""
from __future__ import annotations

import json
import subprocess
from pathlib import Path

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
HERE = Path(__file__).resolve().parent


def tier_kind(fixture: str) -> str:
    t = fixture.split("/")[0]
    return "fail wrong reason" if t == "fail" else "pass/run rejected"


def read_bisect(path: Path) -> dict:
    if path.is_file():
        return json.loads(path.read_text())
    return {}


def main() -> int:
    classify = json.loads((HERE / "pin_classify.json").read_text())
    inp = json.loads((HERE / "part7_input.json").read_text())

    bisects = {
        "family_carrier": read_bisect(HERE / "bisect_family_carrier.json"),
        "countdown_rank": read_bisect(HERE / "bisect_countdown.json"),
        "kernel_subtract": read_bisect(HERE / "bisect_kernel_subtract.json"),
    }

    # PIN clusters (regrouped)
    pin_clusters = classify["pin_clusters"]
    table_rows = []
    for i, c in enumerate(pin_clusters):
        bkey = ["countdown_rank", "family_carrier", "kernel_subtract"][i]
        b = bisects.get(bkey, {})
        brk = b.get("breaking_commit") or ""
        pr = b.get("pr") or ""
        status = b.get("status") or "propagated"
        mech = b.get("mechanism") or ""
        if not mech and status == "propagated":
            mech = b.get("mechanism") or "propagated: see bisect log"
        table_rows.append(
            {
                "rows": len(c["members"]),
                "kind": tier_kind(c["witness"]),
                "diagnostic": c["diag"],
                "witness": c["witness"],
                "breaking_commit": brk,
                "pr": pr,
                "mechanism": mech,
                "status": status,
                "members": c["members"],
            }
        )

    lines = [
        "# Ledger bisect part 7 (k=7, N=40)",
        "",
        f"Measured commit (PIN): `{PIN}`",
        "",
        "## Summary",
        "",
        f"- Legacy report clusters in slice: **{len(inp['clusters'])}** (report indices 7, 47, 87, 127, 167, 207)",
        f"- Fixture rows re-measured: **{classify['counts']['total']}**",
        f"- Fixed on PIN: **{classify['counts']['fixed']}**",
        f"- Still broken on PIN: **{classify['counts']['broken']}**",
        f"- PIN diagnostic clusters (after regroup): **{len(pin_clusters)}**",
        "",
        "## PIN clusters (regrouped by first diagnostic)",
        "",
        "| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |",
        "| ---: | --- | --- | --- | --- | ---: | --- | --- |",
    ]
    for r in table_rows:
        diag = r["diagnostic"].replace("|", "\\|")[:140]
        mech = r["mechanism"].replace("|", "\\|")[:160]
        brk = (r["breaking_commit"] or "")[:12]
        lines.append(
            f"| {r['rows']} | {r['kind']} | {diag} | `{r['witness']}` | `{brk}` | {r['pr']} | {mech} | {r['status']} |"
        )
        lines.append("Members:")
        for m in r["members"]:
            cls = next((x[1] for x in classify["rows"] if x[0] == m), "?")
            fd = next((x[2] for x in classify["rows"] if x[0] == m), "")
            lines.append(f"- `{m}` — {cls}" + (f": {fd[:120]}" if fd else ""))
        lines.append("")

    lines += ["## Fixed on main (PIN)", ""]
    for fix, cls, diag in classify["rows"]:
        if cls == "fixed on main":
            lines.append(f"- `{fix}`")

    lines += [
        "",
        "## Legacy slice members (report cluster 2026-11-11)",
        "",
    ]
    for c in inp["clusters"]:
        lines.append(
            f"### report index {c['slice_index']} (report rows={c['rows']})"
        )
        lines.append(f"- Witness: `{c['witness']}`")
        lines.append(f"- Report diagnostic prefix: {c['diag_table'][:120]}")
        for m in c["members"]:
            cls = next((x[1] for x in classify["rows"] if x[0] == m), "not measured")
            lines.append(f"- `{m}` — {cls}")
        lines.append("")

    lines += [
        "## Provenance",
        "",
        "- Host: linux x86_64",
        "- Jobs: `OMEGA_CORPUS_JOBS=1`",
        "- Commands:",
        "  ```bash",
        "  mbx build -p omega < /dev/null",
        "  mbx test --no-run -p compiler --test corpus_runner < /dev/null",
        "  OMEGA_CORPUS_FIXTURE_FILTER=<fixtures> OMEGA_CORPUS_RECORDS=.agents/ledger-bisect-2026-11-11/pin_records.txt \\",
        "    target/debug/build/compiler/*/out/corpus_runner-* < /dev/null",
        "  ```",
    ]

    (HERE / "part-7.md").write_text("\n".join(lines) + "\n")

    tsv_lines = ["fixture\tclass\tfirst_diagnostic\tcluster_id"]
    for i, c in enumerate(pin_clusters):
        cid = f"pin-{i}"
        for m in c["members"]:
            cls = next((x[1] for x in classify["rows"] if x[0] == m), "unknown")
            fd = next((x[2] for x in classify["rows"] if x[0] == m), "")
            tsv_lines.append(f"{m}\t{cls}\t{fd}\t{cid}")
    for fix, cls, diag in classify["rows"]:
        if cls == "fixed on main":
            tsv_lines.append(f"{fix}\tfixed on main\t{diag}\tfixed")
    (HERE / "part-7.tsv").write_text("\n".join(tsv_lines) + "\n")
    print("wrote part-7.md and part-7.tsv")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
