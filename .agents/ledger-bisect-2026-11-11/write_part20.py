#!/usr/bin/env python3
"""Render part-20.md and part-20.tsv from part20_results.json and rows json."""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT_MD = ROOT / ".agents/ledger-bisect-2026-11-11/part-20.md"
OUT_TSV = ROOT / ".agents/ledger-bisect-2026-11-11/part-20.tsv"
RESULTS = ROOT / ".agents/ledger-bisect-2026-11-11/part20_results.json"
ROWS = Path("/tmp/part20_rows.json")
PIN = "51e8c45c793007f211d0d9baee7de16767172444"


def main() -> None:
    data = json.loads(RESULTS.read_text())
    rows = json.loads(ROWS.read_text()) if ROWS.exists() else []
    lines = [
        "# Ledger bisect part 20",
        "",
        f"PIN: `{PIN}`",
        "",
        "## Re-measure (slice fixtures)",
        "",
        f"- fixed on main: {data['measurement']['fixed']}",
        f"- still broken: {data['measurement']['broken']}",
        f"- total rows: {data['measurement']['fixture_count']}",
        "",
        "## Clusters at PIN (regrouped by first diagnostic)",
        "",
        "| Rows | Kind | Diagnostic (truncated) | Witness | Breaking commit | PR | Mechanism | Status |",
        "| ---: | --- | --- | --- | --- | ---: | --- | --- |",
    ]
    for c in data["clusters"]:
        lines.append(
            f"| {c['rows']} | {c['kind']} | {c['diagnostic'][:80]} | `{c['witness']}` | `{c['breaking_commit'][:12]}` | {c.get('pr','')} | {c['mechanism'][:100]} | {c['status']} |"
        )
    lines += ["", "### Members", ""]
    for c in data["clusters"]:
        lines.append(f"**{c['id']}**")
        for m in c["members"]:
            cls = next((r["class"] for r in rows if r["fixture"] == m), "")
            lines.append(f"- `{m}` ({cls})")
        for fm in c.get("fixed_members", []):
            lines.append(f"- `{fm}` (fixed on main)")
        lines.append("")
    lines += [
        "## Fixed-on-main (all slice members)",
        "",
    ]
    for r in rows:
        if r["class"] == "fixed on main":
            lines.append(f"- `{r['fixture']}`")
    lines += [
        "",
        "## Commands",
        "",
        "```bash",
        "cp tests/omega/corpus_outcomes.txt /tmp/bisect_pin_corpus_outcomes.txt  # at PIN",
        "mbx build -p omega && mbx build -p compiler --bins --tests",
        "corpus_runner with OMEGA_CORPUS_FIXTURE_FILTER (16 fixtures) -> /tmp/part20_pin.txt",
        ".agents/ledger-bisect-2026-11-11/bisect_cluster.sh <witness> <tag>",
        "```",
        "",
        "Host: cloud VM single job, OMEGA_CORPUS_JOBS=1, mbx per-step rebuild.",
        "",
    ]
    OUT_MD.write_text("\n".join(lines) + "\n")
    tsv_lines = []
    cluster_map = {}
    for c in data["clusters"]:
        for m in c["members"]:
            cluster_map[m] = c["id"]
        for m in c.get("fixed_members", []):
            cluster_map[m] = c["id"]
    for r in rows:
        cid = cluster_map.get(r["fixture"], "-")
        diag = r.get("diag", "").replace("\t", " ")
        tsv_lines.append(f"{r['fixture']}\t{r['class']}\t{diag}\t{cid}")
    OUT_TSV.write_text("\n".join(tsv_lines) + "\n")


if __name__ == "__main__":
    main()
