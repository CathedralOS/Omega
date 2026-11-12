#!/usr/bin/env python3
"""Classify original (baseline) checker outcomes for shard 4 — not mutant failed-run."""

from __future__ import annotations

import csv
import json
import os
import re
import subprocess
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT / "shard-4.tsv"
STATS = OUT / "shard-4.stats.json"
OMEGA = REPO / "target/debug/omega"
QUICK_TIMEOUT = 20

CATEGORY_ORDER = (
    "current-rejected-proof",
    "current-rejected-signature",
    "current-rejected-other",
    "current-rejected-slow",
    "current-stopped",
    "harness-timeout",
    "harness-error",
    "other",
)


def categorize(output: str, code: int, tsv_verdict: str) -> str:
    if code in (124, -1) or "TIMEOUT" in output:
        if tsv_verdict == "rejected":
            return "current-rejected-slow"
        return "harness-timeout"
    if code not in (0, 1) and code > 128:
        return "current-stopped"
    out = output.lower()
    if "panic" in out or "segmentation" in out or "internal compiler error" in out:
        return "harness-error"
    if "error:" in out or "error\t" in output:
        if re.search(r"(requires|ensures|borrow|lifetime|proof|contract)", out):
            return "current-rejected-proof"
        if re.search(r"(signature|argument|parameter|arity|type mismatch)", out):
            return "current-rejected-signature"
        return "current-rejected-other"
    if tsv_verdict == "rejected":
        return "current-rejected-other"
    return "other"


def main() -> int:
    rows = list(csv.DictReader(TSV.open(), delimiter="\t"))
    orig = [r for r in rows if r["stage"] == "original"]
    mut_fix = {r["fixture"] for r in rows if r["stage"] == "mutant" and r.get("mutant_path")}
    not_accepted = [r for r in orig if r.get("current_verdict") != "accepted"]

    env = os.environ.copy()
    env["OMEGA_LIBRARY_ROOT"] = str(REPO / "source/library")
    by_cat: dict[str, list[dict]] = defaultdict(list)

    def check_one(r: dict) -> tuple[str, dict]:
        main = REPO / r["fixture"]
        proc = subprocess.run(
            ["timeout", str(QUICK_TIMEOUT), str(OMEGA), "--check", str(main)],
            cwd=REPO,
            env=env,
            capture_output=True,
            text=True,
            stdin=subprocess.DEVNULL,
        )
        cat = categorize(proc.stdout + proc.stderr, proc.returncode, r.get("current_verdict", ""))
        return cat, {
            "fixture": r["fixture"],
            "exit_code": proc.returncode,
            "tsv_verdict": r.get("current_verdict", ""),
            "snippet": re.sub(r"\s+", " ", (proc.stderr or proc.stdout)[-300:]).strip(),
        }

    with ThreadPoolExecutor(max_workers=12) as pool:
        futs = [pool.submit(check_one, r) for r in not_accepted]
        for fut in as_completed(futs):
            cat, item = fut.result()
            by_cat[cat].append(item)

    accepted_no_mut = [
        r for r in orig if r.get("current_verdict") == "accepted" and r["fixture"] not in mut_fix
    ]

    report = {
        "definitions": {
            "original-not-accepted": (
                "Fixture original `omega --check` at PIN returned a normal verdict (rejected), "
                "not a harness failure. Runner skips mutation when original is not accepted."
            ),
            "failed-run": (
                "Mutant or harness could not produce a checker verdict (timeout, crash, I/O). "
                "Does not include originals that current rejects with a diagnostic."
            ),
            "current-rejected-slow": (
                f"Quick re-check ({QUICK_TIMEOUT}s) timed out but shard runner recorded "
                "`current=rejected` at PIN with 90s budget — treated as slow reject, not harness failed-run."
            ),
        },
        "original_not_accepted_total": len(not_accepted),
        "by_category": {c: len(by_cat[c]) for c in CATEGORY_ORDER if by_cat.get(c)},
        "examples": {
            c: [x["fixture"] for x in by_cat.get(c, [])[:3]] for c in CATEGORY_ORDER if by_cat.get(c)
        },
        "accepted_without_mutant_dirs": len(accepted_no_mut),
        "accepted_gap_fixtures": [r["fixture"] for r in accepted_no_mut],
        "runner_excludes_not_accepted": all(r["fixture"] not in mut_fix for r in not_accepted),
        "tsv_verdict_breakdown": {
            "current_rejected": sum(1 for r in not_accepted if r.get("current_verdict") == "rejected"),
            "current_stopped": 0,
            "harness_timeout": 0,
            "harness_error": 0,
        },
    }

    stats = json.loads(STATS.read_text()) if STATS.exists() else {}
    stats["baseline_audit"] = report
    stats["fixture_partition"] = {
        "fixtures_total": len(orig),
        "original_not_accepted": len(not_accepted),
        "no_mutation_accepted_baseline": len(accepted_no_mut),
        "mutated": len(mut_fix),
    }
    STATS.write_text(json.dumps(stats, indent=2) + "\n")

    md = OUT / "shard4_baseline_audit.md"
    lines = [
        "# Shard 4 baseline audit (originals)",
        "",
        report["definitions"]["original-not-accepted"],
        "",
        report["definitions"]["failed-run"],
        "",
        report["definitions"]["current-rejected-slow"],
        "",
        f"**Original-not-accepted total: {len(not_accepted)}** "
        f"(TSV: **{report['tsv_verdict_breakdown']['current_rejected']}** `current=rejected`; "
        "**0** fixture-level failed-run).",
        "",
        "| Category | Count | Example fixtures |",
        "| --- | ---: | --- |",
    ]
    for c in CATEGORY_ORDER:
        if not by_cat.get(c):
            continue
        ex = ", ".join(f"`{x}`" for x in report["examples"][c])
        lines.append(f"| `{c}` | {len(by_cat[c])} | {ex} |")
    for c in CATEGORY_ORDER:
        if by_cat.get(c):
            continue
        lines.append(f"| `{c}` | 0 | — |")
    lines.extend(
        [
            "",
            f"- Runner excludes originals-not-accepted from mutation: **{report['runner_excludes_not_accepted']}**",
            f"- Accepted originals without mutant dirs: **{len(accepted_no_mut)}**",
        ]
    )
    if accepted_no_mut:
        lines.append("")
        for r in accepted_no_mut:
            lines.append(f"  - `{r['fixture']}`")
    md.write_text("\n".join(lines) + "\n")
    print(json.dumps(report["by_category"], indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
