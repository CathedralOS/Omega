#!/usr/bin/env python3
"""Find nearest good first-parent ancestor of PIN (minimum steps from PIN where test==0)."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def is_ledger_only(root: Path, commit: str) -> bool:
    try:
        files = subprocess.check_output(
            ["git", "diff", "--name-only", f"{commit}^", commit],
            cwd=root,
            text=True,
            stderr=subprocess.DEVNULL,
        ).splitlines()
    except subprocess.CalledProcessError:
        return False
    if not files:
        return True
    return all(f == "tests/omega/corpus_outcomes.txt" or f.endswith(".md") for f in files)


def run_at(commit: str, tools: Path, witness: str, root: Path) -> int:
    subprocess.check_call(["git", "checkout", commit, "--quiet"], cwd=root)
    proc = subprocess.run(
        [str(tools / "bisect_witness.sh")],
        cwd=root,
        env={**os.environ, "BISECT_REPO_ROOT": str(root), "BISECT_WITNESS": witness},
    )
    return proc.returncode


def effective_code(root: Path, commit: str, tools: Path, witness: str, cache: dict[str, int]) -> int:
    if commit in cache:
        return cache[commit]
    if is_ledger_only(root, commit):
        cache[commit] = 125
        return 125
    code = run_at(commit, tools, witness, root)
    cache[commit] = code
    return code


def main() -> int:
    root = Path(sys.argv[1])
    pin = sys.argv[2]
    witness = sys.argv[3]
    tools = Path(sys.argv[4])
    commits = subprocess.check_output(
        ["git", "rev-list", "--first-parent", pin],
        cwd=root,
        text=True,
    ).splitlines()
    if not commits:
        return 1
    pin_code = effective_code(root, commits[0], tools, witness, {})
    if pin_code != 1:
        print(f"PIN {commits[0]} did not classify as bad (code={pin_code})", file=sys.stderr)
        return 1
    # Expand index range: find oldest index on chain that is not ledger-only for floor.
    lo, hi = 0, len(commits) - 1
    cache: dict[str, int] = {commits[0]: 1}
    # Assume some ancestor is good (e.g. ledger expects checked); search for minimum index with good.
    best_good: str | None = None
    while lo <= hi:
        mid = (lo + hi) // 2
        c = commits[mid]
        code = effective_code(root, c, tools, witness, cache)
        while code == 125 and mid < hi:
            mid += 1
            c = commits[mid]
            code = effective_code(root, c, tools, witness, cache)
        if code == 0:
            best_good = c
            hi = mid - 1
        elif code == 1:
            lo = mid + 1
        else:
            lo = mid + 1
    if not best_good:
        print("GOOD_NOT_FOUND", file=sys.stderr)
        return 1
    idx = commits.index(best_good)
    subject = subprocess.check_output(
        ["git", "log", "-1", "--format=%s", best_good],
        cwd=root,
        text=True,
    ).strip()
    print(f"GOOD {best_good} first_parent_index={idx} builds={len(cache)} {subject}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
