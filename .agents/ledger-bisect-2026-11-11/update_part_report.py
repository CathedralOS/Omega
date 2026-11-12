#!/usr/bin/env python3
"""Render part-10.md and part-10.tsv from measurement + bisect results."""
from __future__ import annotations

import json
from pathlib import Path

PART = 10
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
BASE = Path(__file__).resolve().parent


def load_json(p: Path) -> dict:
    return json.loads(p.read_text()) if p.exists() else {}


def main() -> None:
    meta = load_json(BASE / "part_meta.json")
    clusters = meta.get("pin_clusters", [])
    fixed = meta.get("fixed_on_main", [])
    results_dir = BASE / "results"
    bisects = {}
    for p in sorted(results_dir.glob("*.json")):
        bisects[p.stem] = load_json(p)

    lines = [
        f"# Ledger bisect part {PART}",
        "",
        f"Measured PIN: `{PIN}`",
        "",
        "## PIN cluster summary",
        "",
        "| cluster | rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | status |",
        "| --- | ---: | --- | --- | --- | --- | --- | --- | --- |",
    ]
    tsv_rows: list[str] = []
    for c in sorted(clusters, key=lambda x: -x["row_count"]):
        r = bisects.get(c["id"], {})
        diag = c["diag_sample"].replace("|", "\\|")[:100]
        mech = (r.get("mechanism") or "").replace("|", "\\|")[:120]
        brk = r.get("breaking_commit") or ""
        lines.append(
            f"| {c['id']} | {c['row_count']} | {c['kind']} | {diag} | `{c['witness']}` | "
            f"`{brk[:12]}` | {r.get('pr') or ''} | {mech} | {r.get('status','pending')} |"
        )
        for m in c["members"]:
            cls = "still_broken" if m in c["members_broken"] else "fixed_on_main"
            fd = c.get("member_diag", {}).get(m, "")
            tsv_rows.append(f"{m}\t{cls}\t{fd}\t{c['id']}")

    lines += ["", "## Members", ""]
    for c in clusters:
        lines.append(f"### {c['id']} ({c['row_count']} rows)")
        for m in c["members"]:
            tag = "fixed" if m in c.get("members_fixed", []) else "broken"
            d = c.get("member_diag", {}).get(m, "")
            lines.append(f"- `{m}` ({tag}) {d[:120]}")
        lines.append("")

    lines += ["## Fixed on PIN (prior slice rows)", ""]
    for f in sorted(fixed):
        lines.append(f"- `{f}`")

    lines += [
        "",
        "## Commands",
        "",
        "- Direct measure: `OMEGA_CORPUS_FIXTURE_FILTER=... corpus_runner`",
        "- Bisect: `.agents/ledger-bisect-2026-11-11/tools/bisect_one_cluster.sh`",
        "- Host: linux x86_64, `OMEGA_CORPUS_JOBS=1`",
    ]

    (BASE / f"part-{PART}.md").write_text("\n".join(lines) + "\n")
    (BASE / f"part-{PART}.tsv").write_text(
        "fixture\tclass\tfirst_diagnostic\tcluster_id\n" + "\n".join(tsv_rows) + "\n"
    )


if __name__ == "__main__":
    main()
