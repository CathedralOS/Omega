#!/usr/bin/env python3
"""Generate ledger regressions audit markdown from measurement artifacts."""
from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
AUDIT = REPO / "wiki/drafts/audits"
REPORT = AUDIT / "ledger_regressions_2026-11-11.md"


def main() -> int:
    measured = json.loads((AUDIT / "measurement.json").read_text())
    clusters = json.loads((AUDIT / "clusters.json").read_text())
    bisects = {}
    bdir = AUDIT / "bisects"
    if bdir.is_dir():
        for p in sorted(bdir.glob("*.json")):
            bisects[p.stem] = json.loads(p.read_text())

    counts = clusters["counts"]
    host = measured.get("host_only", [])
    n_bisected = sum(1 for c in clusters["clusters"] if bisects.get(c["diag_norm"], {}).get("status") == "bisected")
    n_prop = sum(1 for c in clusters["clusters"] if bisects.get(c["diag_norm"], {}).get("status", "propagated") == "propagated")

    lines: list[str] = [
        "# Ledger regression triage (2026-11-11)",
        "",
        "Re-measured sweep regressions on current `main`, clustered by first diagnostic,",
        "with per-cluster `git bisect run --first-parent` witnesses (classification only).",
        "",
        "## Counts",
        "",
        "| Class | Count |",
        "| --- | ---: |",
        f"| fixed on main | {counts['fixed']} |",
        f"| still broken | {counts['broken']} |",
        f"| host only | {counts['host_only']} |",
        f"| missing | {counts['missing']} |",
        f"| total sweep rows | {counts['total']} |",
        "",
        f"Bisected clusters: **{n_bisected}**; propagated (not bisected): **{n_prop}**.",
        "",
    ]
    if host:
        lines += ["## Host-only (timeout under xtask batch cap)", ""]
        for h in host:
            lines.append(f"- `{h}`")
        lines.append("")

    lines += [
        "## Clusters (largest first)",
        "",
        "| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |",
        "| ---: | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for c in clusters["clusters"]:
        b = bisects.get(c["diag_norm"], {})
        kind = "fail accepted" if c["witness"].startswith("fail/") else "pass/run regression"
        diag = c["diag_sample"].replace("|", "\\|")[:120]
        mech = (b.get("mechanism") or "").replace("|", "\\|")[:100]
        lines.append(
            f"| {c['count']} | {kind} | {diag} | `{c['witness']}` | "
            f"`{b.get('breaking_commit', '')[:12]}` | {b.get('pr') or ''} | {mech} | {b.get('status', 'propagated')} |"
        )
        mline = ", ".join(f"`{x}`" for x in c["members"][:30])
        if len(c["members"]) > 30:
            mline += f", … (+{len(c['members']) - 30})"
        lines.append(f"Members: {mline}")
        lines.append("")

    lines += [
        "## Measurement provenance",
        "",
        f"- Measured commit: `{measured['commit']}`",
        f"- Baseline sweep: `759c80404428` (`work/ledger-sweep-regressions`)",
        f"- Host: {measured.get('host_desc', 'linux x86_64')}",
        f"- Jobs: `OMEGA_CORPUS_JOBS=1`",
        f"- Commands: `mbx xtask corpus --filter` batches (≤200); host settle `OMEGA_CORPUS_FIXTURE_SECS=3600`",
        f"- Report generated: {datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M UTC')}",
    ]
    REPORT.write_text("\n".join(lines) + "\n")
    print(f"wrote {REPORT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
