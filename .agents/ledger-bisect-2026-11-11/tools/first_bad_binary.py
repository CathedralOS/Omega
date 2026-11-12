#!/usr/bin/env python3
"""First-parent binary search for first bad commit (pin-frozen witness test)."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
INSTALL = Path("/tmp/ledger-bisect-p6")
WITNESS = sys.argv[1]
GOOD = sys.argv[2]
BAD = sys.argv[3]


def run_witness(sha: str) -> int:
  env = {
    **os.environ,
    "OMEGA_REPO_ROOT": str(ROOT),
    "BISECT_WITNESS": WITNESS,
    "BISECT_PIN_REF": "/tmp/ledger-bisect-p6-ref",
    "BISECT_TEST_PY": str(INSTALL / "bisect_test.py"),
    "CARGO_TARGET_DIR": f"/tmp/bisect-binary-{sha[:8]}",
  }
  subprocess.run(["git", "-C", str(ROOT), "checkout", "-q", sha], check=True)
  rc = subprocess.run([str(INSTALL / "bisect_witness.sh")], env=env).returncode
  return rc


def main() -> int:
  commits = (
    subprocess.check_output(
      ["git", "-C", str(ROOT), "rev-list", "--first-parent", f"{GOOD}..{BAD}"],
      text=True,
    )
    .strip()
    .splitlines()
  )
  commits.reverse()
  if not commits:
    print("empty range", file=sys.stderr)
    return 1

  lo, hi = 0, len(commits) - 1
  cache: dict[str, int] = {}

  def rc_at(i: int) -> int:
    sha = commits[i]
    if sha not in cache:
      cache[sha] = run_witness(sha)
      print(f"probe {sha[:12]} rc={cache[sha]}", flush=True)
    return cache[sha]

  g0 = rc_at(0)
  if g0 != 0:
    print(f"good end {commits[0]} not rc=0 (rc={g0})", file=sys.stderr)
  gn = rc_at(len(commits) - 1)
  if gn != 1:
    print(f"bad end {commits[-1]} not rc=1 (rc={gn})", file=sys.stderr)

  while lo < hi:
    mid = (lo + hi) // 2
    rc = rc_at(mid)
    if rc == 0:
      lo = mid + 1
    elif rc == 1:
      hi = mid
    else:
      # Ambiguous: prefer searching newer half if older was good.
      if mid > 0 and rc_at(mid - 1) == 0:
        lo = mid + 1
      else:
        hi = mid

  first_bad = commits[hi]
  parent = (
    subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", f"{first_bad}^"], text=True)
    .strip()
  )
  subprocess.run(["git", "-C", str(ROOT), "checkout", "-q", "work/ledger-bisect-part-6"], check=False)
  print(first_bad)
  print(f"parent={parent[:12]} parent_rc={run_witness(parent)} first_bad_rc={run_witness(first_bad)}")
  return 0


if __name__ == "__main__":
  raise SystemExit(main())
