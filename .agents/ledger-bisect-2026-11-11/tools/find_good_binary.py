#!/usr/bin/env python3
"""Walk first-parent from PIN until fixed-PIN witness test is good (rc=0)."""
from __future__ import annotations

import os
import subprocess
import sys

REPO = os.environ.get("OMEGA_REPO", "/agent/repos/omega")
TOOLS = os.environ.get("BISECT_TOOLS", "/tmp/ledger-bisect-tools")
sys.path.insert(0, TOOLS)
from bisect_helpers import commit_only_ledger_or_docs, has_corpus_runner_at  # noqa: E402


def probe(witness: str, rev: str) -> int:
    subprocess.check_call(
        ["git", "-C", REPO, "checkout", "-f", rev],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    p = subprocess.run(
        [f"{TOOLS}/run_witness_at_rev.sh", witness],
        env={**os.environ, "OMEGA_REPO": REPO, "BISECT_TOOLS": TOOLS},
        cwd=REPO,
    )
    print(f"probe {rev[:12]} rc={p.returncode}", file=sys.stderr)
    return p.returncode


def main() -> int:
    witness, pin = sys.argv[1], sys.argv[2]
    pin_full = subprocess.check_output(
        ["git", "-C", REPO, "rev-parse", f"{pin}^{{commit}}"], text=True
    ).strip()
    revs = subprocess.check_output(
        ["git", "-C", REPO, "rev-list", "--first-parent", pin_full],
        text=True,
    ).splitlines()
    if probe(witness, pin_full) != 1:
        raise SystemExit(f"PIN must be bad (rc=1) for {witness}")
    good = None
    for rev in revs[1:]:
        if commit_only_ledger_or_docs(rev):
            print(f"skip ledger-only {rev[:12]}", file=sys.stderr)
            continue
        if not has_corpus_runner_at(rev):
            print(f"skip no corpus_runner {rev[:12]}", file=sys.stderr)
            continue
        rc = probe(witness, rev)
        if rc == 0:
            good = rev
            break
        if rc == 1:
            continue
        if rc == 125:
            print(f"skip probe 125 {rev[:12]}", file=sys.stderr)
            continue
        raise SystemExit(f"ambiguous rc={rc} at {rev}")
    subprocess.check_call(
        ["git", "-C", REPO, "checkout", "-f", "work/ledger-bisect-part-11"],
        stdout=subprocess.DEVNULL,
    )
    if good is None:
        raise SystemExit("no good commit found on first-parent walk")
    print(good)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
