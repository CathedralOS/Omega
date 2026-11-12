#!/usr/bin/env python3
"""Walk first-parent from PIN until witness matches expected (bisect_test exit 0)."""
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
    for f in files:
        if f == "tests/omega/corpus_outcomes.txt" or f.endswith(".md"):
            continue
        return False
    return True


def run_at(commit: str, tools: Path, witness: str, root: Path) -> int:
    subprocess.check_call(["git", "checkout", commit, "--quiet"], cwd=root)
    env = os.environ.copy()
    env["BISECT_REPO_ROOT"] = str(root)
    env["BISECT_WITNESS"] = witness
    proc = subprocess.run(
        [str(tools / "bisect_witness.sh")],
        cwd=root,
        env=env,
    )
    return proc.returncode


def main() -> int:
    root = Path(sys.argv[1])
    pin = sys.argv[2]
    witness = sys.argv[3]
    tools = Path(sys.argv[4])
    max_builds = int(os.environ.get("BISECT_FIND_GOOD_MAX", "500"))
    commits = subprocess.check_output(
        ["git", "rev-list", "--first-parent", pin],
        cwd=root,
        text=True,
    ).splitlines()
    builds = 0
    for i, commit in enumerate(commits):
        if builds >= max_builds:
            print(f"GOOD_NOT_FOUND within {max_builds} compiler builds", file=sys.stderr)
            return 1
        if is_ledger_only(root, commit):
            continue
        builds += 1
        print(f"try step={i} build={builds} {commit[:12]}", flush=True)
        code = run_at(commit, tools, witness, root)
        if code == 0:
            subject = subprocess.check_output(
                ["git", "log", "-1", "--format=%s", commit],
                cwd=root,
                text=True,
            ).strip()
            print(f"GOOD {commit} first_parent_steps={i} builds={builds} {subject}")
            return 0
        if code not in (1, 125):
            print(f"unexpected code {code} at {commit}", file=sys.stderr)
    print("GOOD_NOT_FOUND: exhausted first-parent chain", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
