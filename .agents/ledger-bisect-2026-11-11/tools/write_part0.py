#!/usr/bin/env python3
"""Render part-0.md and part-0.tsv from JSON state."""
from __future__ import annotations

import json
import sys
from pathlib import Path

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
OUT_DIR = Path(__file__).resolve().parents[1]


def main() -> int:
    state_path = Path(sys.argv[1])
    state = json.loads(state_path.read_text())
    lines = [
        "# Ledger bisect part 0 (k=0, N=40)",
        "",
        f"Measured commit (PIN): `{PIN}`",
        "",
        "## PIN cluster summary",
        "",
        "| rows | kind | diagnostic | witness | breaking commit | PR | mechanism | status |",
        "| ---: | --- | --- | --- | --- | ---: | --- | --- |",
    ]
    tsv_rows: list[str] = []
    cluster_id = 0
    for cl in state.get("pin_clusters", []):
        cluster_id += 1
        b = cl.get("bisect", {})
        status = b.get("status", "pending")
        mech = b.get("mechanism", "").replace("|", "\\|")
        diag = cl.get("diag", "").replace("|", "\\|")[:120]
        kind = cl.get("kind", "pass/run regression")
        flaky = ""
        if b.get("flaky_good") or b.get("flaky_bad"):
            flaky = " flaky"
        lines.append(
            f"| {cl['count']} | {kind} | {diag} | `{cl['witness']}` | "
            f"`{b.get('first_bad', '')[:12]}` | {b.get('pr', '') or ''} | {mech} | {status}{flaky} |"
        )
        if b.get("good"):
            lines.append(f"Good: `{b['good'][:12]}`")
        for m in cl.get("members", []):
            cls = "broken" if m in cl.get("broken_set", cl.get("members", [])) else "fixed"
            fd = state.get("fixture_diag", {}).get(m, "")
            tsv_rows.append(f"{m}\t{cls}\t{fd}\t{cluster_id}")
        members = " ".join(f"`{x}`" for x in cl.get("members", []))
        lines.append(f"Members: {members}")
        lines.append("")

    lines += [
        "## Prior-slice rows fixed on main",
        "",
    ]
    for fx in state.get("fixed_on_main", []):
        lines.append(f"- `{fx}`")
    lines.append("")
    lines += [
        "## Prior slice member recovery",
        "",
        f"- Prior cluster idx 0 report rows=70; recovered **{state.get('idx0_recovered', 64)}** paths from "
        "`regressed_fixtures.txt` open_const_implementation families (corpus batches incomplete on report branch).",
        "",
        "## Provenance",
        "",
        f"- Host: linux x86_64, `OMEGA_CORPUS_JOBS=1`",
        f"- Commands: `.agents/ledger-bisect-2026-11-11/tools/run_direct_filter.sh`, `git bisect run --first-parent` via `run_bisect_cluster.sh`",
        f"- Bisect worktree: `/tmp/bisect-wt`",
    ]
    (OUT_DIR / "part-0.md").write_text("\n".join(lines) + "\n")
    (OUT_DIR / "part-0.tsv").write_text("\n".join(tsv_rows) + ("\n" if tsv_rows else ""))
    print(f"wrote {OUT_DIR/'part-0.md'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
