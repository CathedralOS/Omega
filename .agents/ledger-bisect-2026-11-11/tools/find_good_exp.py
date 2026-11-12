#!/usr/bin/env python3
"""Nearest good on first-parent: exponential backoff then binary search (walk-back equivalent)."""
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
    return bool(files) and all(
        f == "tests/omega/corpus_outcomes.txt" or f.endswith(".md") for f in files
    )


def test_commit(
    root: Path, tools: Path, witness: str, commit: str, cache: dict[str, int]
) -> int:
    if commit in cache:
        return cache[commit]
    if is_ledger_only(root, commit):
        cache[commit] = 125
        return 125
    subprocess.check_call(["git", "checkout", commit, "--quiet"], cwd=root)
    proc = subprocess.run(
        [str(tools / "bisect_witness.sh")],
        cwd=root,
        env={**os.environ, "BISECT_REPO_ROOT": str(root), "BISECT_WITNESS": witness},
    )
    cache[commit] = proc.returncode
    print(f"  test {commit[:12]} -> {proc.returncode}", flush=True)
    return proc.returncode


def main() -> int:
    root = Path(sys.argv[1])
    pin = sys.argv[2]
    witness = sys.argv[3]
    tools = Path(sys.argv[4])
    commits = subprocess.check_output(
        ["git", "rev-list", "--first-parent", pin], cwd=root, text=True
    ).splitlines()
    n = len(commits) - 1
    cache: dict[str, int] = {}
    if test_commit(root, tools, witness, commits[0], cache) != 1:
        print("PIN is not bad", file=sys.stderr)
        return 1
    last_bad, first_good = 0, None
    i = 0
    jump = 1
    while i < n:
        i = min(i + jump, n)
        code = test_commit(root, tools, witness, commits[i], cache)
        if code == 0:
            first_good = i
            break
        if code == 1:
            last_bad = i
        jump *= 2
    if first_good is None:
        print("GOOD_NOT_FOUND exponential", file=sys.stderr)
        return 1
    lo, hi = last_bad, first_good
    while hi - lo > 1:
        mid = (lo + hi) // 2
        code = test_commit(root, tools, witness, commits[mid], cache)
        if code == 0:
            hi = mid
        elif code == 1:
            lo = mid
        else:
            lo = mid
    good = commits[hi]
    subject = subprocess.check_output(
        ["git", "log", "-1", "--format=%s", good], cwd=root, text=True
    ).strip()
    print(
        f"GOOD {good} index={hi} builds={len(cache)} {subject}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
