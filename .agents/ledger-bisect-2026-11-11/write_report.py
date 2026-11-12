#!/usr/bin/env python3
from __future__ import annotations

import json
import subprocess
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DIR = ROOT / ".agents/ledger-bisect-2026-11-11"
PIN = "51e8c45c793007f211d0d9baee7de16767172444"

PRIOR_BREAK = {
    "machine `_` ensures contract proof fact `_` is disproved by constant arithmetic on the transition arm guarded by `_`": (
        "f3612364ffbb9489d7be1409397df6867fcbebc6",
        "2135",
        "contract proof admission / constant-arithmetic disprove path regressed (prior report cluster)",
    ),
    "integer node `_` does not fit exact carrier `_`": (
        "f3612364ffbb9489d7be1409397df6867fcbebc6",
        "2135",
        "closed const integer carrier check surfaces out-of-range literal at runtime typing",
    ),
    "cannot compare `_` values with `_`: structural equality is synthesized only through a named implementation -- declare `_` to synthesize structural equality, or ": (
        "f3612364ffbb9489d7be1409397df6867fcbebc6",
        "2135",
        "structural `Equatable` synthesis refusal for `Floats` retained-source comparison",
    ),
    "cannot compare against payload-bearing case `_` with `_`: structural equality is synthesized only through a named implementation -- match on the case in a trans": (
        "f3612364ffbb9489d7be1409397df6867fcbebc6",
        "2135",
        "payload-bearing case `==` without named `Equatable` implementation (Chain::Cons)",
    ),
    "cannot prove requires contract for call observe from main: cells[N] in Cell::Issued in state `_`": (
        "f3612364ffbb9489d7be1409397df6867fcbebc6",
        "2135",
        "owned-capture weaker formal: `Cell::Issued` requires not established at observe call",
    ),
    "const parameter `_` of `_` has an ineligible index type: indices require finite structural values with decidable equality and one canonical form": (
        "f3612364ffbb9489d7be1409397df6867fcbebc6",
        "2135",
        "predicate const generic index type ineligible for `Indexed` parameter",
    ),
    "division by zero in closed const argument: the divisor is provably zero, which always traps at runtime. Remove the operation or use a nonzero divisor.": (
        "f3612364ffbb9489d7be1409397df6867fcbebc6",
        "2135",
        "closed const argument division-by-zero retained-source refusal",
    ),
    "generated generic application does not match its exact retained instance origin": (
        "f3612364ffbb9489d7be1409397df6867fcbebc6",
        "2135",
        "generic application vs exact retained instance origin mismatch in build const indexing",
    ),
}


def load_bisect(cid: int) -> dict:
    p = DIR / "bisects" / f"cluster_{cid}.json"
    if p.exists():
        return json.loads(p.read_text())
    return {}


def main() -> None:
    clusters = json.loads((DIR / "pin_clusters.json").read_text())
    tsv = (DIR / "part-4.tsv").read_text()
    lines = [
        "# Ledger bisect part 4 (k=4, N=40)",
        "",
        f"Measured PIN: `{PIN}` ({datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M UTC')})",
        "",
        "## Prior slice summary (report at `741270df60f7`)",
        "",
        "- Six prior clusters indexed 4, 44, 84, 124, 164, 204 (36 ledger rows claimed).",
        "- Only **22** prior member paths recovered from report explicit members plus",
        "  `authored declaration typing…` diagnostic-prefix expansion from archived batch",
        "  observation; **14 rows** from the 27-row authored-declaration cluster lack",
        "  recoverable member paths in archived corpus artifacts (truncated report).",
        "",
        f"- Re-measured **22** / ~36 recoverable prior members on PIN: **10 fixed**, **12 still broken**.",
        "",
        "## Clusters on PIN (still broken regrouped)",
        "",
        "| rows | kind | diagnostic (PIN) | witness | breaking commit | PR | mechanism | status |",
        "| ---: | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for c in clusters:
        b = load_bisect(c["cluster_id"])
        diag = c["diag"].replace("|", "\\|")[:120]
        status = b.get("status", "propagated")
        brk = b.get("breaking_commit") or ""
        pr = b.get("pr") or ""
        mech = b.get("mechanism") or ""
        if status == "propagated" and not brk:
            hint = PRIOR_BREAK.get(c["diag_norm"])
            if hint:
                brk, pr, mech = hint[0][:12], hint[1], hint[2] + " (prior-report propagated; PIN bisect incomplete)"
        lines.append(
            f"| {c['rows']} | {c['kind']} | {diag} | `{c['witness']}` | `{brk[:12]}` | {pr} | {mech[:100].replace('|','/')} | {status} |"
        )
        members = ", ".join(f"`{m}`" for m in c["members"])
        lines.append(f"Members: {members}")
        lines.append("")
    lines += ["## Fixed on PIN", ""]
    for row in tsv.splitlines():
        parts = row.split("\t")
        if len(parts) >= 2 and parts[1] == "fixed_on_main":
            lines.append(f"- `{parts[0]}`")
    lines += [
        "",
        "## Commands",
        "",
        "- `CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_TARGET_DIR=$PWD/target mbx build -p compiler --bins --tests`",
        "- `OMEGA_CORPUS_JOBS=1 OMEGA_CORPUS_FIXTURE_FILTER=<fixtures> OMEGA_CORPUS_RECORDS=.agents/ledger-bisect-2026-11-11/pin_observed.txt corpus_runner`",
        "- Host: linux x86_64 cloud agent VM; jobs: 1",
        "",
    ]
    (DIR / "part-4.md").write_text("\n".join(lines))


if __name__ == "__main__":
    main()
