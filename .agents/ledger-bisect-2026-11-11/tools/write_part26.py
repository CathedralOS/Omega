#!/usr/bin/env python3
"""Emit part-26.md and part-26.tsv from measurement artifacts."""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DIR = Path(__file__).resolve().parents[1]
PIN = "51e8c45c793007f211d0d9baee7de16767172444"


def load_json(p: Path) -> dict:
    return json.loads(p.read_text()) if p.is_file() else {}


def main() -> int:
    clusters = json.loads((DIR / "work/pin_clusters.json").read_text())
    bisects = {}
    for p in sorted((DIR / "work").glob("bisect_c[0-9].json")):
        data = load_json(p)
        if data:
            bisects[data["witness"]] = data
    meta = load_json(DIR / "work/part26_meta.json")

    fixed = meta.get("fixed", [])
    members = meta.get("members", [])
    broken = sum(c["count"] for c in clusters)
    fixed_n = len(fixed)

    lines = [
        "# Ledger bisect part 26",
        "",
        f"Measured commit (PIN): `{PIN}`",
        "",
        "## Slice summary",
        "",
        f"| still broken (reclustered) | {broken} |",
        f"| fixed on main | {fixed_n} |",
        "",
        "## Clusters",
        "",
        "| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |",
        "| ---: | --- | --- | --- | --- | ---: | --- | --- |",
    ]

    tsv_lines = ["fixture\tclass\tfirst_diagnostic\tcluster_id"]

    for idx, c in enumerate(clusters):
        b = bisects.get(c["witness"], {})
        kind = "pass/run regression" if not c["witness"].startswith("fail/") else "fail wrong reason"
        diag = c["diag_sample"].replace("|", "\\|")[:160]
        brk = b.get("breaking_commit", "") or ""
        pr = b.get("pr") or ""
        mech = (b.get("mechanism") or "").replace("|", "\\|")[:160]
        status = b.get("status", "pending")
        lines.append(
            f"| {c['count']} | {kind} | {diag} | `{c['witness']}` | `{brk[:12]}` | {pr} | {mech} | {status} |"
        )
        lines.append("")
        lines.append("Members:")
        for m in c["members"]:
            lines.append(f"- `{m}`")
        if b.get("endpoints"):
            ep = b["endpoints"]
            g = ep.get("good", {})
            p = ep.get("pin", {})
            a = ep.get("anchor", {})
            lines.append(
                f"- endpoints: good={g.get('results')} flaky={g.get('flaky')}; "
                f"pin={p.get('results')} flaky={p.get('flaky')}; "
                f"anchor={a.get('results')} flaky={a.get('flaky')}"
            )
        if b.get("invalid_reason"):
            lines.append(f"- invalid: {b['invalid_reason']}")
        lines.append("")

        for m in c["members"]:
            tsv_lines.append(f"{m}\tstill broken\t{c['diag_sample']}\tc{idx}")

    for m in fixed:
        lines.append(f"- fixed on main: `{m}`")
        tsv_lines.append(f"{m}\tfixed on main\t\tfixed")

    lines += [
        "",
        "## Commands",
        "",
        "- Host: linux x86_64 cloud VM",
        "- Jobs: `OMEGA_CORPUS_JOBS=1`, `CARGO_BUILD_JOBS=8`, `MBX_SCHEDULER_CPUS=8`",
        "- Measure: direct `corpus_runner` with `OMEGA_CORPUS_FIXTURE_FILTER`",
        "- Bisect: `git bisect start --first-parent` + `.agents/ledger-bisect-2026-11-11/tools/bisect_witness_full.sh`",
        "- `git fetch --unshallow origin` before bisect",
    ]

    (DIR / "part-26.md").write_text("\n".join(lines) + "\n")
    (DIR / "part-26.tsv").write_text("\n".join(tsv_lines) + "\n")
    print(f"wrote part-26.md ({broken} broken, {fixed_n} fixed, {len(bisects)} bisects)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
