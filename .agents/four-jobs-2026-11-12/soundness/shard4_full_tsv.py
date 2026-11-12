#!/usr/bin/env python3
"""Rebuild complete shard-4.tsv: every mutation attempt from progress + classifications."""

from __future__ import annotations

import csv
import hashlib
import json
import os
import re
import subprocess
import sys
import time
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from threading import Lock

REPO = Path(__file__).resolve().parents[3]
SHARD = 4
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT / f"shard-{SHARD}.tsv"
PROGRESS = OUT / f"shard-{SHARD}.progress"
MUT_ROOT = OUT / f"shard-{SHARD}"
OMEGA = REPO / "target/debug/omega"
ROUTE = REPO / "target/debug/examples/route_one_comparison"
CHECK_TIMEOUT = 90
WORKERS = 4

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
    "check_time_note",
    "hole_subtype",
    "probe_note",
    "no_mutation_reason",
]

_IO = Lock()


def slug(s: str) -> str:
    h = hashlib.sha256(s.encode()).hexdigest()[:10]
    safe = re.sub(r"[^a-zA-Z0-9_-]+", "_", s)[:80]
    return f"{safe}_{h}"


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


def parse_progress() -> tuple[list[tuple[str, str]], set[str]]:
    attempts: list[tuple[str, str]] = []
    fixtures_done: set[str] = set()
    for ln in PROGRESS.read_text().splitlines():
        if not ln.startswith("fixture:"):
            continue
        body = ln[len("fixture:") :]
        if ":" not in body:
            fixtures_done.add(body)
            continue
        rel, kind = body.rsplit(":", 1)
        attempts.append((rel, kind))
    return attempts, fixtures_done


def run_checker(cmd: list[str]) -> tuple[int, str]:
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
            reason = re.sub(r"[\t\n]+", " ", (m.group(2) or "")).strip()[:400]
    return verdict or "error", reason


def mutant_rel(fixture_rel: str, kind: str) -> str:
    d = MUT_ROOT / slug(f"{fixture_rel}_{kind}")
    return d.relative_to(REPO).as_posix() if d.exists() else ""


def load_classifier():
    final_path = REPO / ".agents/four-jobs-2026-11-12/soundness/shard4_finalize.py"
    ns: dict = {"__file__": str(final_path)}
    exec(compile(final_path.read_text(), str(final_path), "exec"), ns)
    return ns["classify_row"]


def process_attempt(classify_row, fixture_rel: str, kind: str) -> dict:
    tier = tier_of(fixture_rel)
    mp = mutant_rel(fixture_rel, kind)
    row = {
        "fixture": fixture_rel,
        "tier": tier,
        "stage": "mutant",
        "mutation_kind": kind,
        "mutant_path": mp,
        "current_verdict": "",
        "route_verdict": "",
        "route_rejection_reason": "",
        "seconds": "",
        "hand_check": "",
        "spec_rule_file": "",
        "spec_quote": "",
        "bucket": "",
        "obligation_line": "",
        "accepted_by": "",
        "validity_reason": "",
        "no_mutation_reason": "",
    }
    if not mp:
        row["bucket"] = "no-mutation"
        row["no_mutation_reason"] = f"mutation `{kind}` not applicable to source"
        row["hand_check"] = row["bucket"]
        return row
    main = REPO / mp / "main.omg"
    if not main.exists():
        row["bucket"] = "no-mutation"
        row["no_mutation_reason"] = "mutant directory incomplete (no main.omg)"
        row["hand_check"] = row["bucket"]
        return row
    t0 = time.monotonic()
    c_code, _ = run_checker(["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(main)])
    cur = "accepted" if c_code == 0 else "rejected" if c_code != -1 else "timeout"
    row["current_verdict"] = cur
    if cur == "accepted":
        r_code, r_out = run_checker(["timeout", str(CHECK_TIMEOUT), str(ROUTE), str(main)])
        row["route_verdict"], row["route_rejection_reason"] = parse_route(r_out, r_code)
    elif cur == "timeout":
        row["bucket"] = "timeout"
        row["hand_check"] = "timeout"
        row["seconds"] = f"{time.monotonic() - t0:.2f}"
        return row
    else:
        row["route_verdict"] = ""
    row["seconds"] = f"{time.monotonic() - t0:.2f}"
    classified = classify_row(row)
    classified["no_mutation_reason"] = ""
    return classified


def baseline_row(fixture_rel: str) -> dict:
    t0 = time.monotonic()
    code, _ = run_checker(["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(REPO / fixture_rel)])
    return {
        "fixture": fixture_rel,
        "tier": tier_of(fixture_rel),
        "stage": "original",
        "mutation_kind": "",
        "mutant_path": "",
        "current_verdict": "accepted" if code == 0 else "rejected",
        "route_verdict": "",
        "route_rejection_reason": "",
        "seconds": f"{time.monotonic() - t0:.2f}",
        "hand_check": "baseline",
        "bucket": "",
        "no_mutation_reason": "",
    }


def fixture_no_mutation_row(fixture_rel: str, reason: str) -> dict:
    return {
        "fixture": fixture_rel,
        "tier": tier_of(fixture_rel),
        "stage": "no-mutation",
        "mutation_kind": "",
        "mutant_path": "",
        "current_verdict": "",
        "route_verdict": "",
        "route_rejection_reason": "",
        "seconds": "",
        "hand_check": "no-mutation",
        "bucket": "no-mutation",
        "validity_reason": reason,
        "no_mutation_reason": reason,
    }


def main() -> int:
    classify_row = load_classifier()
    attempts, _ = parse_progress()
    print(f"mutation attempts in progress: {len(attempts)}", flush=True)
    fixtures = list_shard_fixtures()
    by_fix: dict[str, list[str]] = defaultdict(list)
    for rel, kind in attempts:
        by_fix[rel].append(kind)

    mut_rows: list[dict] = []
    done = 0
    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        futs = [pool.submit(process_attempt, classify_row, rel, kind) for rel, kind in attempts]
        for fut in as_completed(futs):
            mut_rows.append(fut.result())
            done += 1
            if done % 200 == 0:
                print(f"classified {done}/{len(attempts)}", flush=True)

    mut_rows.sort(key=lambda r: (r["fixture"], r.get("mutation_kind", "")))
    bucket_counts = Counter(r.get("bucket", "?") for r in mut_rows)

    no_mut_fixture_rows: list[dict] = []
    for rel in fixtures:
        kinds = by_fix.get(rel, [])
        generated = sum(1 for k in kinds if mutant_rel(rel, k))
        if generated == 0 and kinds:
            no_mut_fixture_rows.append(
                fixture_no_mutation_row(
                    rel, f"all {len(kinds)} kinds not applicable (no mutant dirs)"
                )
            )
        elif not kinds:
            no_mut_fixture_rows.append(
                fixture_no_mutation_row(rel, "fixture not processed in progress log")
            )

    print("baselines...", flush=True)
    originals: list[dict] = []
    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        for fut in as_completed([pool.submit(baseline_row, rel) for rel in fixtures]):
            originals.append(fut.result())
    originals.sort(key=lambda r: r["fixture"])

    with TSV.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=HEADER, delimiter="\t", extrasaction="ignore")
        w.writeheader()
        w.writerows(originals)
        w.writerows(no_mut_fixture_rows)
        w.writerows(mut_rows)

    stats = {
        "fixtures_in_shard": len(fixtures),
        "mutation_attempts": len(mut_rows),
        "fixture_no_mutation_rows": len(no_mut_fixture_rows),
        "buckets": dict(bucket_counts),
        "mutated_fixtures": len([f for f in fixtures if any(mutant_rel(f, k) for k in by_fix.get(f, []))]),
    }
    (OUT / f"shard-{SHARD}.stats.json").write_text(json.dumps(stats, indent=2) + "\n")
    print("wrote", len(originals), "originals,", len(no_mut_fixture_rows), "fixture no-mutation,", len(mut_rows), "mutants")
    print("buckets", dict(bucket_counts))
    assert len(mut_rows) == len(attempts)
    return 0


if __name__ == "__main__":
    sys.exit(main())
