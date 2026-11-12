#!/usr/bin/env python3
"""Backfill route_rejection_reason and repair TSV columns after runner completes."""

from __future__ import annotations

import csv
import json
import os
import re
import subprocess
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SHARD = 4
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT / f"shard-{SHARD}.tsv"
STATS = OUT / f"shard-{SHARD}.stats.json"
PROGRESS = OUT / f"shard-{SHARD}.progress"
ROUTE = REPO / "target/debug/examples/route_one_comparison"
CHECK_TIMEOUT = 90


def run_checker(cmd: list[str]) -> tuple[int, str]:
    env = os.environ.copy()
    env["OMEGA_LIBRARY_ROOT"] = str(REPO / "source/library")
    try:
        proc = subprocess.run(
            cmd,
            cwd=REPO,
            env=env,
            capture_output=True,
            text=True,
            timeout=CHECK_TIMEOUT,
            stdin=subprocess.DEVNULL,
        )
        return proc.returncode, proc.stdout + proc.stderr
    except subprocess.TimeoutExpired:
        return -1, "TIMEOUT"


def parse_route(output: str, code: int) -> str:
    if code == -1:
        return "timeout"
    for line in output.splitlines():
        if line.startswith("program\t"):
            m = re.search(r"\broute=([^\s]+)", line)
            if m:
                return m.group(1)
    return "error"


def parse_route_reason(output: str, code: int) -> str:
    if code == -1:
        return "timeout"
    for line in output.splitlines():
        if line.startswith("program\t"):
            m = re.search(r"\broute=(\S+)\s*(.*)", line)
            if m and m.group(2).strip():
                return m.group(2).strip().replace("\t", " ")[:400]
    for line in output.splitlines():
        if line.startswith("diagnostic\t") or line.startswith("fallback\t"):
            parts = line.split("\t")
            if len(parts) >= 6:
                return parts[-1][:400]
    return ""


def progress_mutant_keys() -> list[tuple[str, str]]:
    """(fixture_rel, kind) from progress file."""
    out: list[tuple[str, str]] = []
    if not PROGRESS.exists():
        return out
    for ln in PROGRESS.read_text().splitlines():
        if ":drop_" not in ln and not re.search(r":(widen_|remove_|strip_|ref_|delete_)", ln):
            continue
        if not ln.startswith("fixture:"):
            continue
        body = ln[len("fixture:") :]
        if ":" not in body:
            continue
        rel, kind = body.rsplit(":", 1)
        out.append((rel, kind))
    return out


def enrich_row(row: dict) -> dict:
    if row.get("stage") != "mutant":
        return row
    mp = row.get("mutant_path", "")
    if not mp:
        return row
    path = REPO / mp
    if not path.exists():
        return row
    if row.get("route_rejection_reason"):
        return row
    if row.get("current_verdict") != "accepted":
        return row
    r_code, r_out = run_checker(
        ["timeout", str(CHECK_TIMEOUT), str(ROUTE), str(path)]
    )
    row["route_verdict"] = parse_route(r_out, r_code)
    row["route_rejection_reason"] = parse_route_reason(r_out, r_code)
    return row


def compute_stats(rows: list[dict], progress_keys: list[tuple[str, str]]) -> dict:
    c: Counter[str] = Counter()
    c["mutants_attempted"] = len(progress_keys)
    c["mutants_generated"] = len([k for k in progress_keys])  # each key attempted apply
    logged = sum(1 for r in rows if r.get("stage") == "mutant")
    c["mutants_logged_tsv"] = logged
    for r in rows:
        if r.get("stage") != "mutant":
            continue
        cur = r.get("current_verdict", "")
        if cur == "rejected":
            c["mutants_current_rejected"] += 1
        elif cur == "timeout":
            c["mutants_failed_run"] += 1
        elif cur:
            c["mutants_run"] += 1
    c["mutants_not_applicable"] = max(
        0, c["mutants_attempted"] - c.get("mutants_generated", c["mutants_attempted"])
    )
    if STATS.exists():
        try:
            prior = json.loads(STATS.read_text())
            c["runner_stats_note"] = "merged with runner JSON at enrich time"
            for k, v in prior.items():
                if k not in c:
                    c[k] = v
        except json.JSONDecodeError:
            pass
    c["mutant_total_accounted"] = (
        c["mutants_logged_tsv"]
        + c.get("mutants_failed_run", 0)
        + max(0, c["mutants_attempted"] - c["mutants_logged_tsv"] - c.get("mutants_failed_run", 0))
    )
    return dict(c)


def main() -> None:
    rows = []
    if TSV.exists():
        with TSV.open() as f:
            rows = list(csv.DictReader(f, delimiter="\t"))
    fieldnames = [
        "fixture",
        "tier",
        "stage",
        "mutation_kind",
        "mutant_path",
        "current_verdict",
        "route_verdict",
        "route_rejection_reason",
        "seconds",
        "hand_check",
        "spec_rule_file",
        "spec_quote",
        "bucket",
        "obligation_line",
        "accepted_by",
        "validity_reason",
        "hole_subtype",
        "probe_note",
    ]
    out_rows: list[dict] = []
    for r in rows:
        row = dict(r)
        if row.get("stage") == "mutant":
            row = enrich_row(row)
        out_rows.append(row)
    progress_keys = progress_mutant_keys()
    stats = compute_stats(out_rows, progress_keys)
    STATS.write_text(json.dumps(stats, indent=2) + "\n")
    with TSV.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fieldnames, delimiter="\t", extrasaction="ignore")
        w.writeheader()
        w.writerows(out_rows)
    print("enriched rows", len(out_rows), "stats", stats)


if __name__ == "__main__":
    main()
