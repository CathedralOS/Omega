#!/usr/bin/env python3
import json
import subprocess
import sys
from pathlib import Path

PIN = "51e8c45c793007f211d0d9baee7de16767172444"
DIR = Path(__file__).resolve().parent
WT = DIR.parents[1] / ".agents/ledger-bisect-wt"


def ledger_only(sha: str) -> bool:
    st = subprocess.run(
        ["git", "-C", str(WT), "diff-tree", "--no-commit-id", "--name-only", "-r", sha],
        capture_output=True,
        text=True,
    )
    files = [f for f in st.stdout.splitlines() if f.strip()]
    if not files:
        return False
    return all(f == "tests/omega/corpus_outcomes.txt" or f.endswith(".md") for f in files)


def valid(prev: dict) -> bool:
    if prev.get("find_good_method") != "exponential":
        return False
    m = prev.get("mechanism") or ""
    if m.startswith("never-good since"):
        return len(prev.get("fixture_added") or m.split()[-1]) == 40
    if prev.get("status") != "bisected":
        return False
    brk = prev.get("breaking_commit") or ""
    if len(brk) != 40 or brk == PIN or ledger_only(brk):
        return False
    return bool(prev.get("breaking_exit_triplet"))


def main() -> int:
    n = 12
    ok = 0
    for i in range(n):
        p = DIR / "bisects" / f"cluster_{i}.json"
        if not p.exists():
            print(i, "missing")
            continue
        d = json.loads(p.read_text())
        if valid(d):
            ok += 1
            print(i, "ok", d["breaking_commit"][:12], d.get("pr"))
        else:
            print(i, "pending", d.get("status"), (d.get("breaking_commit") or "")[:12])
    print(f"valid={ok}/{n}")
    return 0 if ok == n else 1


if __name__ == "__main__":
    raise SystemExit(main())
