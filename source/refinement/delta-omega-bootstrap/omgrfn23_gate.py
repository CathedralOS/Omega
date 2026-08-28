#!/usr/bin/env python3
"""Lower-rooted OMGRFN23 reference orchestration."""

from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
GATES = HERE.parents[2] / "source/on-ramp/omega-bootstrap/gates"


def reference_gate() -> None:
    started = time.monotonic()
    env = dict(os.environ, PYTHONPATH=f"{HERE}:{GATES}")
    subprocess.run(
        [sys.executable, "-B", str(HERE / "omgrfn23_owner_test.py")],
        check=True,
        env=env,
    )
    python_elapsed = time.monotonic() - started
    beta_started = time.monotonic()
    subprocess.run([str(HERE / "omgrfn23-beta-join.sh")], check=True)
    print(
        f"OMGRFN23 reference integration: modular Python owners {python_elapsed:.2f}s; "
        f"split persisted-Beta projections {time.monotonic() - beta_started:.2f}s"
    )


if __name__ == "__main__":
    try:
        reference_gate()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"OMGRFN23 reference integration: {error}", file=sys.stderr)
        raise SystemExit(1)
