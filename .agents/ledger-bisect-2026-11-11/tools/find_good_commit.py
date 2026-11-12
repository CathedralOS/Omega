#!/usr/bin/env python3
"""Newest first-parent ancestor of PIN where frozen witness test returns 0."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

REPO = Path("/agent/repos/omega")
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
ANCHOR = "741270df60f7"
TOOLS = Path("/tmp/ledger_bisect_tools_2026_11_11")
if not TOOLS.is_dir():
    TOOLS = Path(__file__).resolve().parent


def run_test(witness: str) -> int:
    r = subprocess.run(
        ["bash", str(TOOLS / "bisect_witness.sh")],
        cwd=str(REPO),
        env={**dict(__import__("os").environ), "BISECT_WITNESS": witness, "OMEGA_REPO": str(REPO)},
    )
    return r.returncode


def rev_list_first_parent(pin: str) -> list[str]:
    r = subprocess.run(
        ["git", "-C", str(REPO), "rev-list", "--first-parent", pin],
        capture_output=True,
        text=True,
        check=True,
    )
    return [x for x in r.stdout.splitlines() if x]


def fixture_path(witness: str) -> str:
    tier, rest = witness.split("/", 1)
    return f"tests/omega/{tier}/{rest}"


def oldest_fixture_commit(witness: str) -> str | None:
    r = subprocess.run(
        [
            "git",
            "-C",
            str(REPO),
            "log",
            "--first-parent",
            "--reverse",
            "-1",
            "--format=%H",
            "--",
            fixture_path(witness),
        ],
        capture_output=True,
        text=True,
    )
    return r.stdout.strip() or None


def checkout(ref: str) -> None:
    subprocess.run(["git", "-C", str(REPO), "checkout", "-f", ref], check=True)


def restore_branch() -> None:
    subprocess.run(
        ["git", "-C", str(REPO), "checkout", "-f", "work/ledger-bisect-part-10"],
        check=False,
    )


def main() -> int:
    witness = sys.argv[1]
    revs = rev_list_first_parent(PIN)
    if len(revs) < 2:
        return 1

    checkout(revs[0])
    pin_ec = run_test(witness)
    if pin_ec == 0:
        print(revs[0])
        restore_branch()
        return 0
    if pin_ec == 125:
        print("witness test skip at PIN", file=sys.stderr)
        restore_branch()
        return 1

    try:
        anc = subprocess.run(
            ["git", "-C", str(REPO), "rev-parse", ANCHOR],
            capture_output=True,
            text=True,
        ).stdout.strip()
        lo = revs.index(anc) + 1 if anc in revs else 1
    except Exception:
        lo = 1
    floor = oldest_fixture_commit(witness)
    hi = revs.index(floor) if floor and floor in revs else len(revs) - 1
    hi = min(hi, lo + 1200)

    best = None
    while lo <= hi:
        mid = (lo + hi) // 2
        checkout(revs[mid])
        ec = run_test(witness)
        if ec == 0:
            best = revs[mid]
            hi = mid - 1
        else:
            lo = mid + 1
    restore_branch()
    if not best:
        print("no good commit on first-parent chain", file=sys.stderr)
        return 1
    print(best)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
