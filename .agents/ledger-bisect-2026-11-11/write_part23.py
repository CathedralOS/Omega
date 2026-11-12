#!/usr/bin/env python3
"""Render part-23.md and part-23.tsv from cluster results + pin_observed."""
from __future__ import annotations

import json
from pathlib import Path

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / ".agents/ledger-bisect-2026-11-11"
OBS = (OUT / "pin_observed.txt").read_text().splitlines()
MEMBERS = (OUT / "part23_members.txt").read_text().splitlines()
RESULTS = json.loads((OUT / "part23_results.json").read_text()) if (OUT / "part23_results.json").exists() else {"clusters": []}
LEDGER = Path("/tmp/ledger_bisect_part23_corpus_outcomes.txt").read_text()


def obs_for(fix: str) -> tuple[str, str]:
    for i, line in enumerate(OBS):
        if line.startswith(fix + " "):
            st = line.split()[1]
            d = ""
            if i + 1 < len(OBS) and OBS[i + 1].startswith("\t"):
                d = OBS[i + 1][1:].strip()[:120]
            return st, d
    return "missing", ""


def expected(fix: str) -> str:
    for line in LEDGER.splitlines():
        if line.startswith(fix + " "):
            reg = next((t for t in line.split() if t.startswith("regressed-from:")), None)
            if reg:
                return reg.split(":", 1)[1].split("+")[0]
            if fix.startswith("fail/"):
                return "rejected"
            return "checked"
    return "?"


def broken(fix: str) -> bool:
    st, _ = obs_for(fix)
    exp = expected(fix)
    if fix.startswith("fail/"):
        return st != "rejected"
    return st not in ("checked", "built")


broken_ct = sum(1 for m in MEMBERS if broken(m))
fixed_ct = len(MEMBERS) - broken_ct

lines = [
    "# Ledger bisect part 23 (k=23, N=40)",
    "",
    f"Measured PIN: `{PIN}`",
    "",
    f"Rows in slice: **{len(MEMBERS)}** — fixed on PIN (expected outcome): **{fixed_ct}**, still broken: **{broken_ct}**.",
    "",
    "## Cluster table",
    "",
    "| rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | status |",
    "| ---: | --- | --- | --- | --- | ---: | --- | --- |",
]
for c in RESULTS["clusters"]:
    lines.append(
        f"| {c['rows']} | {c['kind']} | {c['diag'][:100].replace('|','/')} | `{c['witness']}` | "
        f"`{c.get('breaking_commit','')[:12]}` | {c.get('pr','') or ''} | {c.get('mechanism','')[:90].replace('|','/')} | {c.get('status','pending')} |"
    )
lines += ["", "## Members", ""]
by_cluster = {c["cluster_id"]: c for c in RESULTS["clusters"]}
for c in RESULTS["clusters"]:
    lines.append(f"### {c['cluster_id']} ({c['rows']} rows)")
    for m in c.get("members", []):
        st, d = obs_for(m)
        flag = "broken" if broken(m) else "fixed"
        lines.append(f"- `{m}` — {flag} — obs `{st}` — {d[:100]}")
    lines.append("")
lines += [
    "## Fixed-on-main members (cluster-level)",
    "",
]
for c in RESULTS["clusters"]:
    fixed_m = [m for m in c.get("members", []) if not broken(m)]
    if fixed_m:
        lines.append(f"- **{c['cluster_id']}**: " + ", ".join(f"`{x}`" for x in fixed_m))
lines += [
    "",
    "## Commands",
    "",
    "- Host: linux x86_64",
    "- Jobs: `OMEGA_CORPUS_JOBS=1`; macOS `check-target` batches use `OMEGA_CORPUS_NATIVE=1` + `OMEGA_CORPUS_NATIVE_TARGET=macos_arm64`",
    "- PIN ledger copy: `/tmp/ledger_bisect_part23_corpus_outcomes.txt`",
    "- Direct runner: `.agents/ledger-bisect-2026-11-11/measure_fixtures.sh`",
    "- Bisect: `.agents/ledger-bisect-2026-11-11/run_cluster_bisect.sh` with fixed PIN witness specs",
    "",
]
(OUT / "part-23.md").write_text("\n".join(lines) + "\n")

# tsv
rows = []
for c in RESULTS["clusters"]:
    for m in c.get("members", []):
        st, d = obs_for(m)
        cls = "broken" if broken(m) else "fixed"
        rows.append(f"{m}\t{cls}\t{d[:200]}\t{c['cluster_id']}")
(OUT / "part-23.tsv").write_text("\n".join(rows) + "\n")
print(f"wrote part-23.md ({len(RESULTS['clusters'])} clusters)")
