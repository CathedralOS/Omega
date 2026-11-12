#!/usr/bin/env python3
"""Append triple exit-code runs and cluster membership to bisect result JSON."""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
DIR = REPO / ".agents/ledger-bisect-2026-11-11"
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
MEASURE = DIR / "slice_2_pin_measure.txt"
SCRIPT = Path("/tmp/ledger_bisect_tools/bisect_witness.sh")


def run_triple(witness: str, rev: str) -> list[int]:
    env = {**dict(__import__("os").environ), "BISECT_WITNESS": witness, "BISECT_TOOLS_ROOT": "/tmp/ledger_bisect_tools"}
    rcs: list[int] = []
    subprocess.run(
        ["git", "-C", str(REPO), "checkout", "-f", "--detach", rev],
        check=False,
        capture_output=True,
    )
    for _ in range(3):
        p = subprocess.run([str(SCRIPT)], env=env, cwd=REPO)
        rcs.append(p.returncode)
    return rcs


def main() -> int:
    subprocess.run(["bash", str(DIR / "install_bisect_tools.sh")], cwd=REPO, check=False)
    result_path = Path(sys.argv[1])
    witness = sys.argv[2]
    data = json.loads(result_path.read_text())
    brk = data.get("breaking_commit") or ""
    if not brk or data.get("status") not in ("bisected",):
        return 0
    parent = (
        subprocess.run(
            ["git", "-C", str(REPO), "rev-parse", f"{brk}^1"],
            capture_output=True,
            text=True,
        ).stdout.strip()
    )
    data["exit_codes_first_bad"] = run_triple(witness, brk)
    data["exit_codes_first_parent"] = run_triple(witness, parent) if parent else []
    if not (data.get("mechanism") or "").strip():
        data["mechanism"] = "mechanism not established"
    clusters = json.loads((DIR / "slice_2_pin_analysis.json").read_text())["clusters"]
    membership: dict[str, list[int]] = {}
    for c in clusters:
        if witness not in c.get("members", []):
            continue
        if c["count"] < 2:
            break
        for m in c["members"]:
            snap = subprocess.run(
                [sys.executable, str(DIR / "tools/pin_snapshot.py"), m, str(MEASURE)],
                cwd=REPO,
                capture_output=True,
                text=True,
            )
            if snap.returncode != 0:
                continue
            env = {
                **dict(__import__("os").environ),
                "BISECT_WITNESS": m,
                "BISECT_TOOLS_ROOT": "/tmp/ledger_bisect_tools",
            }
            subprocess.run(["git", "-C", str(REPO), "checkout", "-f", "--detach", PIN], capture_output=True)
            rcs = []
            for _ in range(3):
                p = subprocess.run([str(SCRIPT)], env=env, cwd=REPO)
                rcs.append(p.returncode)
            membership[m] = rcs
        data["membership_pin_check"] = membership
        break
    result_path.write_text(json.dumps(data, indent=2) + "\n")
    subprocess.run(["git", "-C", str(REPO), "checkout", "-f", "work/ledger-bisect-part-2"], capture_output=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
