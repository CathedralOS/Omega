#!/usr/bin/env python3
"""Render part-16.md and part-16.tsv from measurement + bisect JSON artifacts."""
from __future__ import annotations

import json
import subprocess
from datetime import datetime, timezone
from pathlib import Path

PART = 16
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
REPO = Path(__file__).resolve().parents[2]
OUT = REPO / ".agents/ledger-bisect-2026-11-11"


def load_json(p: Path) -> dict:
    return json.loads(p.read_text()) if p.is_file() else {}


def main() -> None:
    meta = load_json(OUT / "part16_meta.json")
    clusters = meta.get("pin_clusters", [])
    bisects = meta.get("bisects", {})
    fixed = meta.get("fixed", [])
    members = meta.get("members", {})
    tsv_lines = meta.get("tsv", [])

    lines = [
        f"# Ledger bisect part {PART} (k=16, N=40)",
        "",
        f"Measured commit (PIN): `{PIN}`",
        "",
        "## PIN clusters (still-broken regrouped by first diagnostic)",
        "",
        "| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |",
        "| ---: | --- | --- | --- | --- | ---: | --- | --- |",
    ]
    for i, c in enumerate(clusters):
        b = bisects.get(c["cluster_id"], {})
        mech = (b.get("mechanism") or c.get("mechanism_propagated") or "").replace("|", "\\|")[:200]
        diag = c["diag_sample"].replace("|", "\\|")[:120]
        pr = b.get("pr") or ""
        brk = (b.get("breaking_commit") or "")[:12]
        lines.append(
            f"| {c['count']} | {c['kind']} | {diag} | `{c['witness']}` | `{brk}` | {pr} | {mech} | {b.get('status', 'pending')} |"
        )
        for m in c.get("members", []):
            lines.append(f"- `{m}`")
        lines.append("")

    lines += [
        "## Fixed on main (within slice clusters)",
        "",
    ]
    for m in sorted(fixed):
        lines.append(f"- `{m}`")

    lines += [
        "",
        "## Prior-report slice clusters (index % 40 == 16)",
        "",
        f"- Fixed rows: {meta.get('slice_fixed_count', 0)}; still broken at PIN: {meta.get('slice_broken_count', 0)}",
        "",
        "## Commands",
        "",
        "```bash",
        f"git fetch --unshallow origin",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p omega < /dev/null",
        "CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx test -p compiler --test corpus_runner --no-run < /dev/null",
        "# OMEGA_CORPUS_FIXTURE_FILTER + OMEGA_CORPUS_RECORDS direct corpus_runner (17 fixtures)",
        ".agents/ledger-bisect-2026-11-11/tools/run_bisect_pin.sh <witness> <out.json>",
        "```",
        f"- Host: linux x86_64, jobs: OMEGA_CORPUS_JOBS=1",
        f"- Updated: {datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M UTC')}",
    ]

    (OUT / f"part-{PART}.md").write_text("\n".join(lines) + "\n")
    (OUT / f"part-{PART}.tsv").write_text("".join(tsv_lines) if tsv_lines else "fixture\tclass\tfirst_diagnostic\tcluster_id\n")
    print(f"wrote part-{PART}.md ({len(clusters)} clusters)")


if __name__ == "__main__":
    main()
