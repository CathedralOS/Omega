#!/usr/bin/env python3
"""Rebuild shard-4.tsv after column corruption from enrich/finalize."""

from __future__ import annotations

import csv
import os
import re
import subprocess
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
SHARD = 4
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT / f"shard-{SHARD}.tsv"
OMEGA = REPO / "target/debug/omega"
ROUTE = REPO / "target/debug/examples/route_one_comparison"
CHECK_TIMEOUT = 90

HEADER = [
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


def list_shard_fixtures() -> list[str]:
    proc = subprocess.run(
        "git ls-files 'tests/omega/pass/**/main.omg' 'tests/omega/run/**/main.omg' | LC_ALL=C sort",
        cwd=REPO,
        shell=True,
        capture_output=True,
        text=True,
        check=True,
    )
    lines = sorted(proc.stdout.splitlines(), key=str.lower)
    return [lines[i] for i in range(len(lines)) if i % 8 == SHARD]


def tier_of(rel: str) -> str:
    return rel.split("/")[2]


def run(cmd: list[str]) -> tuple[int, str]:
    env = os.environ.copy()
    env["OMEGA_LIBRARY_ROOT"] = str(REPO / "source/library")
    try:
        p = subprocess.run(
            cmd,
            cwd=REPO,
            env=env,
            capture_output=True,
            text=True,
            timeout=CHECK_TIMEOUT,
            stdin=subprocess.DEVNULL,
        )
        return p.returncode, p.stdout + p.stderr
    except subprocess.TimeoutExpired:
        return -1, "TIMEOUT"


def parse_route(output: str, code: int) -> tuple[str, str]:
    if code == -1:
        return "timeout", "timeout"
    verdict, reason = "", ""
    for line in output.splitlines():
        if not line.startswith("program\t"):
            continue
        m = re.search(r"\broute=(\S+)(?:\s+(.*))?", line)
        if m:
            verdict = m.group(1)
            reason = (m.group(2) or "").strip()[:400]
    if not verdict:
        verdict = "error"
    return verdict, reason


def parse_corrupt_mutants(text: str) -> list[dict]:
    rows: list[dict] = []
    for line in text.splitlines():
        if "\tmutant\t" not in line:
            continue
        fx = re.search(r"(tests/omega/[^\t]+/main\.omg)", line)
        mp = re.search(r"(\.agents/four-jobs-2026-11-12/soundness/shard-4/[^\t]+)", line)
        kind_m = re.search(
            r"/(drop_requires|drop_mut_let|widen_requires_lt|remove_transition_guard|strip_and_guard|ref_to_mut_param|delete_divisor_guard)\b",
            line,
        )
        if not kind_m:
            kind_m = re.search(
                r"\t(drop_requires|drop_mut_let|widen_requires_lt|remove_transition_guard|strip_and_guard|ref_to_mut_param|delete_divisor_guard)\t",
                line,
            )
        if not (fx and mp and kind_m):
            continue
        fixture = fx.group(1)
        mutant_path = mp.group(1)
        kind = kind_m.group(1)
        route_m = re.search(r"\t(rejected|accepted|unreached|no-entry|outside|incomplete|fallback|timeout)\t", line)
        cur_m = re.search(r"\tfailed-run\t(accepted)\t", line) or re.search(r"\t(accepted)\t", line)
        rows.append(
            {
                "fixture": fixture,
                "tier": tier_of(fixture),
                "stage": "mutant",
                "mutation_kind": kind,
                "mutant_path": mutant_path,
                "current_verdict": cur_m.group(1) if cur_m else "accepted",
                "route_verdict": route_m.group(1) if route_m else "",
                "route_rejection_reason": "",
                "seconds": "",
                "hand_check": "pending",
                "spec_rule_file": "",
                "spec_quote": "",
            }
        )
    # dedupe by mutant_path
    seen: set[str] = set()
    out: list[dict] = []
    for r in rows:
        if r["mutant_path"] in seen:
            continue
        seen.add(r["mutant_path"])
        out.append(r)
    return out


def refresh_verdicts(row: dict) -> None:
    mp = REPO / row["mutant_path"] / "main.omg"
    if not mp.exists():
        return
    t0 = time.monotonic()
    c_code, _ = run(["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(mp)])
    row["current_verdict"] = "accepted" if c_code == 0 else "rejected" if c_code != -1 else "timeout"
    if row["current_verdict"] == "accepted":
        r_code, r_out = run(["timeout", str(CHECK_TIMEOUT), str(ROUTE), str(mp)])
        row["route_verdict"], row["route_rejection_reason"] = parse_route(r_out, r_code)
    row["seconds"] = f"{time.monotonic() - t0:.2f}"


def main() -> None:
    corrupt = TSV.read_text() if TSV.exists() else ""
    mutants = parse_corrupt_mutants(corrupt)
    print(f"parsed {len(mutants)} mutant rows from corrupt TSV")
    for i, r in enumerate(mutants):
        if i % 20 == 0:
            print(f"refresh {i}/{len(mutants)}")
        refresh_verdicts(r)
    def baseline(rel: str) -> dict:
        t0 = time.monotonic()
        code, _ = run(["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(REPO / rel)])
        return {
            "fixture": rel,
            "tier": tier_of(rel),
            "stage": "original",
            "mutation_kind": "",
            "mutant_path": "",
            "current_verdict": "accepted" if code == 0 else "rejected",
            "route_verdict": "",
            "route_rejection_reason": "",
            "seconds": f"{time.monotonic() - t0:.2f}",
            "hand_check": "baseline",
            "spec_rule_file": "",
            "spec_quote": "",
        }

    fixtures = list_shard_fixtures()
    originals: list[dict] = []
    with ThreadPoolExecutor(max_workers=4) as pool:
        futs = {pool.submit(baseline, rel): rel for rel in fixtures}
        for fut in as_completed(futs):
            originals.append(fut.result())
    originals.sort(key=lambda r: r["fixture"])
    with TSV.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=HEADER, delimiter="\t", extrasaction="ignore")
        w.writeheader()
        w.writerows(originals)
        w.writerows(mutants)
    print(f"wrote {len(originals)} originals + {len(mutants)} mutants")


if __name__ == "__main__":
    main()
