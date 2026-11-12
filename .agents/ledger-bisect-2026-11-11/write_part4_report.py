#!/usr/bin/env python3
"""Write part-4.md and part-4.tsv from current artifacts."""
from __future__ import annotations

import json
from pathlib import Path

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
DIR = Path(__file__).resolve().parent


def main() -> None:
    prior = json.loads((DIR / "prior_slice4.json").read_text())
    clusters = json.loads((DIR / "pin_clusters.json").read_text()) if (DIR / "pin_clusters.json").exists() else []
    tsv_lines = []
    if (DIR / "part-4.tsv").exists():
        tsv_lines = (DIR / "part-4.tsv").read_text().splitlines()
    lines = [
        "# Ledger bisect part 4 (k=4, N=40)",
        "",
        f"PIN: `{PIN}`",
        "",
        "## Prior slice (report at `741270df60f7`, markdown 226 clusters)",
        "",
        f"- Prior cluster indices: {prior['prior_slice_indices']}",
        f"- Recovered **{len(prior['fixtures'])}** member paths (incl. truncated cluster rows).",
        "",
    ]
    pf = pinf = 0
    if (DIR / "pin_observed.txt").exists():
        import sys

        sys.path.insert(0, str(DIR / "tools"))
        from classify_corpus import parse_records, is_worse_than_ledger

        golden = parse_records((DIR.parents[1] / "tests/omega/corpus_outcomes.txt").read_text())
        obs = parse_records((DIR / "pin_observed.txt").read_text())
        for fix in prior["fixtures"]:
            if fix not in obs:
                continue
            o = obs[fix]
            g = golden.get(fix)
            raw = next((l for l in (DIR / "pin_observed.txt").read_text().splitlines() if l.startswith(fix + " ")), "")
            bad = g and (is_worse_than_ledger(g, o) or (o.get("expected_satisfied") is False and "unexpected" in raw))
            if bad:
                pinf += 1
            else:
                pf += 1
        lines.append(f"- Re-measured on PIN: **{pf} fixed**, **{pinf} still broken**.")
        lines.append("")
    lines += [
        "## PIN clusters (still broken, by first diagnostic)",
        "",
        "| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status | stability |",
        "| ---: | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    fixture_cluster = {}
    for c in clusters:
        b = {}
        bp = DIR / "bisects" / f"cluster_{c['cluster_id']}.json"
        if bp.exists():
            b = json.loads(bp.read_text())
        diag = c["diag"].replace("|", "\\|")[:110]
        brk = (b.get("breaking_commit") or "")[:12]
        lines.append(
            f"| {c['rows']} | {c['kind']} | {diag} | `{c['witness']}` | `{brk}` | {b.get('pr') or ''} | "
            f"{(b.get('mechanism') or '')[:90].replace('|','/')} | {b.get('status','pending')} | {b.get('stability','')} |"
        )
        m = ", ".join(f"`{x}`" for x in c["members"])
        lines.append(f"Members: {m}")
        lines.append("")
        for m in c["members"]:
            fixture_cluster[m] = c["cluster_id"]
    lines += ["## Member lines (PIN)", ""]
    if tsv_lines:
        for ln in tsv_lines:
            lines.append(f"- `{ln.split(chr(9))[0]}` — {ln.split(chr(9))[1] if chr(9) in ln else ''}")
    lines += [
        "",
        "## Commands",
        "- `mbx build -p compiler --bins --tests`; direct `corpus_runner` with `OMEGA_CORPUS_FIXTURE_FILTER`",
        "- Host linux x86_64; `OMEGA_CORPUS_JOBS=1`",
        "",
    ]
    (DIR / "part-4.md").write_text("\n".join(lines))


if __name__ == "__main__":
    main()
