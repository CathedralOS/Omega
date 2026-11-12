#!/usr/bin/env python3
"""Resolve ledger @stamp to a good bisect commit and verify witness exit codes."""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(subprocess.check_output(["git", "rev-parse", "--show-toplevel"], text=True).strip())
GOLDEN = REPO / "tests/omega/corpus_outcomes.txt"
SCRIPT = REPO / ".agents/ledger-bisect-2026-11-11/tools/bisect_witness.sh"


def ledger_stamp(fixture: str) -> str | None:
    for line in GOLDEN.read_text().splitlines():
        if line.startswith(fixture + " "):
            m = re.search(r"@([0-9a-f]+)", line)
            return m.group(1) if m else None
    return None


def stamp_to_commit(stamp: str) -> str | None:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", f"{stamp}^{{commit}}"],
            cwd=REPO,
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
    except subprocess.CalledProcessError:
        pass
    out = subprocess.check_output(
        [
            "git",
            "log",
            "--all",
            "-S",
            f"@{stamp}",
            "--format=%H",
            "-1",
            "--",
            "tests/omega/corpus_outcomes.txt",
        ],
        cwd=REPO,
        text=True,
    ).strip()
    return out or None


def runs(witness: str, commit: str, n: int = 3) -> dict[str, int]:
    subprocess.check_call(["git", "checkout", "-q", commit], cwd=REPO)
    env = {**dict(__import__("os").environ), "BISECT_WITNESS": witness}
    counts = {"ok": 0, "bad": 0, "skip": 0}
    for _ in range(n):
        p = subprocess.run([str(SCRIPT)], cwd=REPO, env=env)
        if p.returncode == 0:
            counts["ok"] += 1
        elif p.returncode == 1:
            counts["bad"] += 1
        else:
            counts["skip"] += 1
    return counts


def main() -> int:
    pin = "51e8c45c793007f211d0d9baee7de16767172444"
    mid = "741270df60f7"
    for witness in sys.argv[1:]:
        stamp = ledger_stamp(witness)
        good = stamp_to_commit(stamp) if stamp else None
        print(f"{witness}")
        print(f"  stamp={stamp} good={good}")
        for label, c in [("good", good), ("mid", mid), ("pin", pin)]:
            if not c:
                continue
            c = subprocess.check_output(["git", "rev-parse", c], cwd=REPO, text=True).strip()
            counts = runs(witness, c, 3)
            subj = subprocess.check_output(["git", "log", "-1", "--oneline", c], cwd=REPO, text=True).strip()
            print(f"  {label} {subj} -> {counts}")
    subprocess.check_call(["git", "checkout", "-q", "work/ledger-bisect-part-30"], cwd=REPO)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
