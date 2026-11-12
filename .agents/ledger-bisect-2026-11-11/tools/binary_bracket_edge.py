#!/usr/bin/env python3
"""Binary search first-parent chain for 0->1 transition (commit-scoped verify)."""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DIR = Path(__file__).resolve().parents[1]
TOOL = Path("/tmp/ledger-bisect-tools/verify_at_commit.sh")
MEASURE = DIR / "work/pin_measure.txt"
PIN = "51e8c45c793007f211d0d9baee7de16767172444"


def sh(*a: str) -> str:
    return subprocess.check_output(a, text=True, cwd=ROOT).strip()


def ledger_only(sha: str) -> bool:
    d = subprocess.run(
        ["git", "diff", "--name-only", f"{sha}^1", sha],
        capture_output=True,
        text=True,
        cwd=ROOT,
    )
    files = [ln for ln in (d.stdout or "").splitlines() if ln.strip()]
    if not files:
        return False
    return all(
        f == "tests/omega/corpus_outcomes.txt" or f.endswith(".md") for f in files
    )


def rc(witness: str, sha: str) -> int:
    if ledger_only(sha):
        return -2
    subprocess.run(
        ["python3", "/tmp/ledger-bisect-tools/pin_bisect_context.py", witness, str(MEASURE)],
        cwd=ROOT,
        check=True,
        stdout=subprocess.DEVNULL,
    )
    out = subprocess.check_output([str(TOOL), sha, witness, "1"], text=True, cwd=ROOT)
    return json.loads(out)["results"][0]


def chain(good: str) -> list[str]:
    rev = sh("git", "rev-list", "--first-parent", PIN).splitlines()
    rev.reverse()
    good = sh("git", "rev-parse", good)
    i = rev.index(good)
    return rev[i:]


def main() -> int:
    good, witness = sys.argv[1], sys.argv[2]
    commits = chain(good)
    cache: dict[str, int] = {}
    skips: list[dict] = []

    def get(i: int) -> int:
        sha = commits[i]
        if sha not in cache:
            try:
                cache[sha] = rc(witness, sha)
            except Exception as exc:  # noqa: BLE001
                cache[sha] = 125
                skips.append({"sha": sha, "reason": str(exc)})
        return cache[sha]

    pin_i = len(commits) - 1
    if get(pin_i) != 1:
        print(json.dumps({"error": "PIN not bad", "rc": get(pin_i)}))
        return 2

    last_good_i = 0
    for i in range(pin_i, -1, -1):
        if get(i) == 0:
            last_good_i = i
            break
        if get(i) == 125:
            skips.append({"sha": commits[i], "reason": "125_at_pin_walk"})

    first_bad_i = pin_i
    for i in range(last_good_i + 1, pin_i + 1):
        if get(i) == 1:
            first_bad_i = i
            break

    edge_good = commits[last_good_i]
    edge_bad = commits[first_bad_i]
    between = commits[last_good_i + 1 : first_bad_i]
    mid_skip = []
    for s in between:
        idx = commits.index(s)
        if get(idx) in (-2, 125):
            mid_skip.append(s)
    out = {
        "edge_good": edge_good,
        "edge_bad": edge_bad,
        "edge_parent": sh("git", "rev-parse", f"{edge_bad}^1"),
        "good_anchor": sh("git", "rev-parse", good),
        "between_shas": between,
        "skipped_between": mid_skip,
        "isolated": len(mid_skip) == 0 and first_bad_i == last_good_i + 1,
        "witness_rc_cache": {k: cache[k] for k in cache},
        "skips": skips,
    }
    if not out["isolated"]:
        out["verdict"] = "first-bad within range, not isolated"
    (DIR / f"work/bracket_{witness.split('/')[-1]}.json").write_text(
        json.dumps(out, indent=2) + "\n"
    )
    print(json.dumps(out, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
