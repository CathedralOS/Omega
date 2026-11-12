#!/usr/bin/env python3
"""Binary search last good on first-parent line using fixed-PIN witness test."""
from __future__ import annotations

import os
import subprocess
import sys

REPO = os.environ.get("OMEGA_REPO", "/agent/repos/omega")
TOOLS = os.environ.get("BISECT_TOOLS", "/tmp/ledger-bisect-tools")
sys.path.insert(0, TOOLS)
from bisect_helpers import commit_only_ledger_or_docs  # noqa: E402

_CACHE: dict[tuple[str, str], int] = {}


def probe(witness: str, rev: str) -> int:
    key = (witness, rev)
    if key in _CACHE:
        return _CACHE[key]
    if commit_only_ledger_or_docs(rev):
        _CACHE[key] = -1
        print(f"probe {rev[:12]} ledger-only skip", file=sys.stderr)
        return -1
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
    _CACHE[key] = p.returncode
    print(f"probe {rev[:12]} rc={p.returncode}", file=sys.stderr)
    return p.returncode


def is_bad(witness: str, rev: str) -> bool | None:
    code = probe(witness, rev)
    if code == -1:
        return None
    if code == 1:
        return True
    if code == 0:
        return False
    raise SystemExit(f"ambiguous rc={code} for {witness} at {rev}")


def main() -> int:
    witness, pin = sys.argv[1], sys.argv[2]
    pin_full = subprocess.check_output(
        ["git", "-C", REPO, "rev-parse", f"{pin}^{{commit}}"], text=True
    ).strip()
    revs = [
        r
        for r in reversed(
            subprocess.check_output(
                ["git", "-C", REPO, "rev-list", "--first-parent", pin_full],
                text=True,
            ).splitlines()
        )
        if not commit_only_ledger_or_docs(r)
    ]
    if not revs:
        return 1
    # pin must be bad
    pin_bad = is_bad(witness, pin_full)
    if pin_bad is not True:
        raise SystemExit(f"PIN is not bad for {witness} (got {pin_bad})")
    lo, hi = 0, len(revs) - 1
    first_bad = len(revs)
    while lo <= hi:
        mid = (lo + hi) // 2
        mid_rev = revs[mid]
        bad = is_bad(witness, mid_rev)
        if bad is None:
            hi = mid - 1
            continue
        if bad:
            first_bad = mid
            hi = mid - 1
        else:
            lo = mid + 1
    if first_bad == 0:
        raise SystemExit("witness bad from oldest first-parent")
    good = revs[first_bad - 1]
    if probe(witness, good) != 0:
        raise SystemExit(f"good commit failed verify {good[:12]}")
    subprocess.check_call(
        ["git", "-C", REPO, "checkout", "-f", "work/ledger-bisect-part-11"],
        stdout=subprocess.DEVNULL,
    )
    print(good)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
