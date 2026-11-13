#!/usr/bin/env python3
"""Verify parent/bad edge and member flip table for one cluster."""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DIR = Path(__file__).resolve().parents[1]
WORK = Path(os.environ.get("LEDGER_BISECT_WORK", DIR / "work"))
TOOLCACHE = Path("/tmp/ledger-bisect-tools")
shutil.copytree(DIR / "tools", TOOLCACHE, dirs_exist_ok=True)
TOOL = TOOLCACHE / "verify_at_commit.sh"
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
MEASURE = WORK / "pin_measure.txt"


def sh(*args: str) -> str:
    return subprocess.check_output(args, text=True, cwd=ROOT).strip()


def verify(commit: str, witness: str, trials: int = 1) -> dict:
    env = {**os.environ, "LEDGER_BISECT_WORK": str(WORK)}
    proc = subprocess.run(
        [str(TOOL), commit, witness, str(trials)],
        text=True,
        cwd=ROOT,
        capture_output=True,
        env=env,
    )
    if not proc.stdout.strip():
        raise RuntimeError(proc.stderr or f"verify rc={proc.returncode}")
    return json.loads(proc.stdout)


def hunk_quote(commit: str, cluster: int) -> str:
    paths = ["omega-rust/psi/"]
    if cluster == 1:
        paths = ["omega-rust/psi/compilation/pipeline/lowered-discharge/"]
    diff = subprocess.run(
        ["git", "show", commit, "--format=", "-U3", "--", *paths],
        capture_output=True,
        text=True,
        cwd=ROOT,
    )
    text = (diff.stdout or "").strip()
    return text[:12000] if text else "mechanism not established"


def main() -> int:
    cluster = int(sys.argv[1])
    bjson = WORK / f"bisect_c{cluster}.json"
    data = json.loads(bjson.read_text())
    bad = data["breaking_commit"]
    if not bad:
        print("no breaking commit", file=sys.stderr)
        return 2
    parent = sh("git", "rev-parse", f"{bad}^1")
    good = sh("git", "rev-parse", data["good"]) if data.get("good") else ""
    witness = data["witness"]
    clusters = json.loads((WORK / "pin_clusters.json").read_text())
    members = clusters[cluster]["members"]
    pr = ""
    subj = sh("git", "log", "-1", "--format=%s", bad)
    if "#" in subj:
        import re

        m = re.search(r"#(\d+)", subj)
        if m:
            pr = m.group(1)
    parent_trials = verify(parent, witness, 3)
    bad_trials = verify(bad, witness, 3)
    member_rows = []
    for m in members:
        member_rows.append(
            {
                "fixture": m,
                "parent": verify(parent, m, 1),
                "bad": verify(bad, m, 1),
            }
        )
    bracket_name = witness.split("/")[-1]
    bracket_path = WORK / f"bracket_{bracket_name}.json"
    bracket = {}
    if bracket_path.is_file():
        bracket = json.loads(bracket_path.read_text())
    out = {
        "cluster": cluster,
        "witness": witness,
        "parent": parent,
        "first_bad": bad,
        "good_anchor": good,
        "pin": PIN,
        "pr": pr or None,
        "parent_trials": parent_trials,
        "bad_trials": bad_trials,
        "members": member_rows,
        "mechanism_hunk_excerpt": hunk_quote(bad, cluster),
        "bracket": bracket or None,
    }
    path = WORK / f"verify_c{cluster}.json"
    path.write_text(json.dumps(out, indent=2) + "\n")
    print(f"wrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
