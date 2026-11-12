#!/usr/bin/env python3
"""Walk first-parent ancestors of PIN until witness test returns 0."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

PIN = os.environ.get("BISECT_PIN", "51e8c45c793007f211d0d9baee7de16767172444")


def run_test(repo: Path, script: Path, witness: str) -> int:
    env = os.environ.copy()
    env["BISECT_WITNESS"] = witness
    env["OMEGA_REPO_ROOT"] = str(repo)
    return subprocess.run([str(script)], env=env, cwd=repo).returncode


def main() -> int:
    witness = sys.argv[1]
    repo = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
    script = Path(os.environ["BISECT_WITNESS_SCRIPT"])
    max_steps = int(os.environ.get("BISECT_GOOD_MAX", "120"))

    commits = subprocess.check_output(
        ["git", "rev-list", "--first-parent", "-n", str(max_steps), PIN],
        cwd=repo,
        text=True,
    ).splitlines()

    branch = subprocess.check_output(["git", "rev-parse", "--abbrev-ref", "HEAD"], cwd=repo, text=True).strip()

    for commit in commits:
        subprocess.check_call(["git", "checkout", "-q", commit], cwd=repo)
        rc = run_test(repo, script, witness)
        if rc == 0:
            subprocess.check_call(["git", "checkout", "-q", branch], cwd=repo)
            print(commit)
            return 0
    subprocess.check_call(["git", "checkout", "-q", branch], cwd=repo)
    print(commits[-1] if commits else PIN)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
