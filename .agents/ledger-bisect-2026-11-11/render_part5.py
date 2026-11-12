#!/usr/bin/env python3
"""Render part-5.md and part-5.tsv."""
from __future__ import annotations

import json
import subprocess
from pathlib import Path

BASE = Path(__file__).resolve().parent
PIN = "51e8c45c793007f211d0d9baee7de16767172444"


def main() -> int:
    pin = json.loads((BASE / "pin_clusters_full.json").read_text())
    prior = json.loads((BASE / "part-5-report-clusters.json").read_text())
    stmt = json.loads((BASE / "bisect_statement_successor.json").read_text())
    dom = json.loads((BASE / "bisect_domain_pattern.json").read_text())
    host = subprocess.check_output(["uname", "-a"], text=True).strip()

    lines = [
        "# Ledger bisect part 5",
        "",
        f"PIN: `{PIN}`",
        "",
        "## Prior slice (cluster indices 5, 45, 85, 125, 165, 205 on report #2359)",
        "",
        "| prior rows | measured members | fixed at PIN | still broken at PIN |",
        "| ---: | ---: | ---: | ---: |",
        f"| 34 (recovered; index-5 row (+22) via report batch corpus) | {pin['counts']['total']} | {pin['counts']['fixed']} | {pin['counts']['broken']} |",
        "",
        "At PIN these rows re-cluster into **2** still-broken diagnostics (11 rows); **23** prior-slice rows are fixed on main.",
        "",
        "## Still-broken clusters at PIN",
        "",
        "| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |",
        "| ---: | --- | --- | --- | --- | --- | --- | --- |",
    ]
    clusters = [
        (pin["clusters"][0], stmt),
        (pin["clusters"][1], dom),
    ]
    for c, b in clusters:
        kind = "pass/run" if c["witness"].startswith("pass/") else "fail"
        diag = c["diag_sample"].replace("|", "\\|")[:110]
        lines.append(
            f"| {c['count']} | {kind} | {diag} | `{c['witness']}` | `{b['breaking_commit'][:12]}` | {b.get('pr','')} | {b.get('mechanism','')[:100]} | {b.get('status')} |"
        )
        lines.append("")
        lines.append("Members:")
        for m in c["members"]:
            cls = "fixed" if m in pin["fixed"] else "broken"
            lines.append(f"- `{m}` ({cls} at PIN)")
        lines.append("")

    lines += ["## Fixed on main at PIN (prior-slice rows)", ""]
    for f in sorted(pin["fixed"]):
        lines.append(f"- `{f}`")
    lines.append("")
    lines += [
        "## Prior slice rows (report indices)",
        "",
    ]
    for p in prior:
        lines.append(
            f"- index {p['cluster_id']}: {p['prior_rows']} rows, witness `{p['witness']}` ({p['members_truncated']} truncated → {len(p.get('members_all', p['members']))} recovered)"
        )
    lines.append("")
    lines += [
        "## Commands",
        "",
        f"- Host: `{host}`",
        "- Jobs: `OMEGA_CORPUS_JOBS=1`, direct `corpus_runner` via `.agents/ledger-bisect-2026-11-11/run_pin_corpus.sh`",
        "- Bisect: fixed `/tmp/ledger-bisect-corpus_outcomes.txt` + `/tmp/ledger-bisect-witness.json`; skip ledger-only commits (`corpus_outcomes.txt` / `*.md` only); good via first-parent walk",
        "- Statement-successor good: `5c6c16d4d1867bcac99343285c5074169d7cbfbc`",
        "- Domain-pattern narrow good: `12a1feb4aa100ab41afd8cc59f53969c2ffc4009`",
    ]
    (BASE / "part-5.md").write_text("\n".join(lines) + "\n")

    tsv = ["fixture\tclass\tfirst_diagnostic\tcluster_id"]
    for c in pin["clusters"]:
        for m in c["members"]:
            cls = "fixed_at_pin" if m in pin["fixed"] else "broken_at_pin"
            diag = c["diag_norm"] if cls == "broken_at_pin" else "(fixed)"
            tsv.append(f"{m}\t{cls}\t{diag}\t{c['id']}")
    (BASE / "part-5.tsv").write_text("\n".join(tsv) + "\n")
    print((BASE / "part-5.md").read_text()[:400])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
