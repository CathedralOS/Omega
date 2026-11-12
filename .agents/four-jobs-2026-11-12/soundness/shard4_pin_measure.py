#!/usr/bin/env python3
"""Re-measure shard 4 at PIN: baselines (300s) and route backfill (--every-machine, 300s)."""

from __future__ import annotations

import argparse
import csv
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
PIN = os.environ.get("SHARD4_PIN", "0835b3aaf803ed97f905be42d94727ff58459263")
SHARD = 4
OUT = REPO / ".agents/four-jobs-2026-11-12/soundness"
TSV = OUT / f"shard-{SHARD}.tsv"
STATS = OUT / f"shard-{SHARD}.stats.json"
BASELINE_CKPT = OUT / "shard-4.baseline-pin.progress"
ROUTE_CKPT = OUT / "shard-4.route-backfill.progress"
CHECK_TIMEOUT = int(os.environ.get("SHARD4_CHECK_TIMEOUT", "300"))
WORKERS = int(os.environ.get("SHARD4_WORKERS", "6"))

OMEGA = Path(os.environ.get("SHARD4_OMEGA", REPO / "target/debug/omega"))
ROUTE = Path(os.environ.get("SHARD4_ROUTE", REPO / "target/debug/examples/route_one_comparison"))

_IO = Lock()


def load_ckpt(path: Path) -> set[str]:
    if not path.exists():
        return set()
    return {ln.strip() for ln in path.read_text().splitlines() if ln.strip()}


def mark_ckpt(path: Path, key: str) -> None:
    with _IO:
        done = load_ckpt(path)
        if key in done:
            return
        with path.open("a") as f:
            f.write(key + "\n")


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


def one_line(text: str, limit: int = 400) -> str:
    return re.sub(r"[\t\n]+", " ", text).strip()[:limit]


def first_diagnostic(output: str) -> str:
    for line in output.splitlines():
        if line.startswith("error\t") or line.startswith("diagnostic\t"):
            parts = line.split("\t")
            if len(parts) >= 2:
                return one_line(parts[-1], 200)
        if re.search(r"\berror:", line, re.I):
            return one_line(line, 200)
    for line in output.splitlines():
        if "error" in line.lower() and len(line.strip()) > 8:
            return one_line(line, 200)
    return ""


def classify_baseline(code: int, output: str) -> tuple[str, str]:
    if code in (124, -1) or "TIMEOUT" in output:
        return "timeout", ""
    if code not in (0, 1) and code > 128:
        return "stopped", f"exit={code}"
    diag = first_diagnostic(output)
    if code == 0:
        return "accepted", ""
    if diag:
        return "rejected-with-diagnostic", diag
    return "error", one_line(output, 200) or f"exit={code}"


def runner_env() -> dict[str, str]:
    env = os.environ.copy()
    env["OMEGA_LIBRARY_ROOT"] = str(REPO / "source/library")
    env["OMEGA_CORPUS_ROOT"] = str(REPO)
    return env


def run_omega(main: Path) -> tuple[int, str, float]:
    env = runner_env()
    t0 = time.monotonic()
    try:
        p = subprocess.run(
            ["timeout", str(CHECK_TIMEOUT), str(OMEGA), "--check", str(main)],
            cwd=REPO,
            env=env,
            capture_output=True,
            text=True,
            stdin=subprocess.DEVNULL,
        )
        return p.returncode, p.stdout + p.stderr, time.monotonic() - t0
    except OSError as exc:
        return -2, str(exc), time.monotonic() - t0


def parse_route_output(output: str, code: int) -> tuple[str, str]:
    if code in (124, -1):
        return "timeout", "timeout"
    verdict, reason = "", ""
    program_lines = [ln for ln in output.splitlines() if ln.startswith("program\t")]
    pick = next((ln for ln in program_lines if "root=compilation" in ln), "")
    if not pick and program_lines:
        pick = program_lines[0]
    for line in program_lines:
        if "root=compilation" in line:
            pick = line
    if pick:
        m = re.search(r"\broute=(\S+)(?:\s+(.*))?", pick)
        if m:
            verdict = m.group(1)
            reason = one_line(m.group(2) or "")
            if verdict == "panicked":
                return "failed-run", reason or "route checker panicked"
    if not verdict:
        return "failed-run", one_line(output, 300) or f"exit={code}"
    if not reason:
        for line in output.splitlines():
            if line.startswith("diagnostic\t") or line.startswith("fallback\t"):
                parts = line.split("\t")
                if len(parts) >= 2:
                    reason = one_line(parts[-1])
                    break
    return verdict, reason


def run_route(main: Path) -> tuple[str, str, float, int]:
    env = runner_env()
    t0 = time.monotonic()
    try:
        p = subprocess.run(
            [
                "timeout",
                str(CHECK_TIMEOUT),
                str(ROUTE),
                "--every-machine",
                str(main),
            ],
            cwd=REPO,
            env=env,
            capture_output=True,
            text=True,
            stdin=subprocess.DEVNULL,
        )
        v, r = parse_route_output(p.stdout + p.stderr, p.returncode)
        return v, r, time.monotonic() - t0, p.returncode
    except OSError as exc:
        return "failed-run", str(exc), time.monotonic() - t0, -2


def load_rows() -> list[dict]:
    with TSV.open() as f:
        return list(csv.DictReader(f, delimiter="\t"))


def write_rows(rows: list[dict], fieldnames: list[str]) -> None:
    with TSV.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fieldnames, delimiter="\t", extrasaction="ignore")
        w.writeheader()
        w.writerows(rows)


def cmd_build() -> int:
    wt = Path("/tmp/omega-pin-shard4")
    if not (wt / ".git").exists():
        subprocess.run(
            ["git", "worktree", "add", "--detach", str(wt), PIN],
            cwd=REPO,
            check=True,
        )
    target = Path("/tmp/omega-pin-shard4-target")
    env = {**os.environ, "CARGO_TARGET_DIR": str(target)}
    subprocess.run(
        [
            "mbx",
            "build",
            "-p",
            "omega",
            "--bins",
            "-p",
            "compiler",
            "--example",
            "route_one_comparison",
        ],
        cwd=wt,
        env=env,
        check=True,
        stdin=subprocess.DEVNULL,
    )
    print("PIN build OK", target / "debug/omega", flush=True)
    return 0


def cmd_baselines(_: argparse.Namespace) -> int:
    if not OMEGA.is_file():
        print(f"missing omega {OMEGA}", file=sys.stderr)
        return 1
    rows = load_rows()
    fieldnames = list(rows[0].keys()) if rows else []
    for col in ("baseline_category", "baseline_diagnostic", "pin_measured"):
        if col not in fieldnames:
            fieldnames.append(col)
    orig_by = {r["fixture"]: r for r in rows if r.get("stage") == "original"}
    fixtures = list_shard_fixtures()
    done = load_ckpt(BASELINE_CKPT)
    updates: dict[str, dict] = {}

    def work(rel: str) -> tuple[str, dict]:
        if rel in done:
            return rel, {}
        code, out, sec = run_omega(REPO / rel)
        cat, diag = classify_baseline(code, out)
        cur = {
            "accepted": "accepted",
            "rejected-with-diagnostic": "rejected",
            "timeout": "timeout",
            "stopped": "error",
            "error": "error",
        }[cat]
        mark_ckpt(BASELINE_CKPT, rel)
        return rel, {
            "current_verdict": cur,
            "baseline_category": cat,
            "baseline_diagnostic": diag,
            "pin_measured": PIN,
            "seconds": f"{sec:.2f}",
        }

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        futs = [pool.submit(work, rel) for rel in fixtures]
        for fut in as_completed(futs):
            rel, upd = fut.result()
            if upd:
                updates[rel] = upd

    for r in rows:
        if r.get("stage") != "original":
            continue
        if r["fixture"] in updates:
            r.update(updates[r["fixture"]])

    write_rows(rows, fieldnames)
    stats = json.loads(STATS.read_text()) if STATS.exists() else {}
    cats = Counter(orig_by.get(f, {}).get("baseline_category", "") for f in fixtures)
    for rel, upd in updates.items():
        cats[upd["baseline_category"]] += 1
    # recount from rows
    cats = Counter(
        r.get("baseline_category", "")
        for r in rows
        if r.get("stage") == "original"
    )
    stats["pin_baseline_rerun"] = {
        "pin": PIN,
        "timeout_sec": CHECK_TIMEOUT,
        "by_category": dict(cats),
        "examples": defaultdict(list),
    }
    for r in rows:
        if r.get("stage") != "original":
            continue
        c = r.get("baseline_category", "")
        if c and len(stats["pin_baseline_rerun"]["examples"][c]) < 3:
            stats["pin_baseline_rerun"]["examples"][c].append(
                {"fixture": r["fixture"], "diagnostic": r.get("baseline_diagnostic", "")}
            )
    stats["pin_baseline_rerun"]["examples"] = dict(stats["pin_baseline_rerun"]["examples"])
    STATS.write_text(json.dumps(stats, indent=2) + "\n")
    print("baseline categories", dict(cats), flush=True)
    return 0


def cmd_routes(_: argparse.Namespace) -> int:
    if not ROUTE.is_file():
        print(f"missing route {ROUTE}", file=sys.stderr)
        return 1
    rows = load_rows()
    fieldnames = list(rows[0].keys()) if rows else []
    for col in ("route_ran_at_pin",):
        if col not in fieldnames:
            fieldnames.append(col)
    mutants = [r for r in rows if r.get("stage") == "mutant" and r.get("mutant_path")]
    done = load_ckpt(ROUTE_CKPT)

    def work(row: dict) -> tuple[str, str, str, str]:
        key = f"{row['fixture']}:{row['mutation_kind']}"
        mp = REPO / row["mutant_path"]
        main = mp / "main.omg" if mp.is_dir() else mp
        if key in done and row.get("route_verdict"):
            return key, row.get("current_verdict", ""), row.get("route_verdict", ""), row.get("route_rejection_reason", "")
        route_v, reason, _, _ = run_route(main)
        need_cur = not row.get("route_verdict") or row.get("current_verdict") in ("", "rejected")
        if need_cur:
            ccode, cout, _ = run_omega(main)
            cat, _ = classify_baseline(ccode, cout)
            if cat == "accepted":
                cur_v = "accepted"
            elif cat == "rejected-with-diagnostic":
                cur_v = "rejected"
            elif cat == "timeout":
                cur_v = "timeout"
            elif cat == "stopped":
                cur_v = "error"
            else:
                cur_v = "error"
        else:
            cur_v = row.get("current_verdict", "")
        mark_ckpt(ROUTE_CKPT, key)
        return key, cur_v, route_v, reason

    updates: dict[str, tuple[str, str, str]] = {}
    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        futs = [pool.submit(work, r) for r in mutants]
        done_n = 0
        for fut in as_completed(futs):
            key, cur, route, reason = fut.result()
            updates[key] = (cur, route, reason)
            done_n += 1
            if done_n % 50 == 0:
                print(f"route backfill {done_n}/{len(mutants)}", flush=True)

    for r in mutants:
        key = f"{r['fixture']}:{r['mutation_kind']}"
        if key not in updates:
            continue
        cur, route, reason = updates[key]
        r["current_verdict"] = cur
        r["route_verdict"] = route
        r["route_rejection_reason"] = reason
        r["route_ran_at_pin"] = PIN
        r["bucket"] = ""
        r["hand_check"] = ""

    write_rows(rows, fieldnames)
    missing = sum(1 for r in mutants if not r.get("route_verdict"))
    print(f"mutants={len(mutants)} missing_route_after={missing}", flush=True)
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("build")
    p1 = sub.add_parser("baselines")
    p2 = sub.add_parser("routes")
    args = ap.parse_args()
    if args.cmd == "build":
        return cmd_build()
    if args.cmd == "baselines":
        return cmd_baselines(args)
    if args.cmd == "routes":
        return cmd_routes(args)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
