#!/usr/bin/env python3
"""Merge /tmp/bisect_<cluster_tag>.log into part20_results.json and render part-20.*"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RESULTS = ROOT / ".agents/ledger-bisect-2026-11-11/part20_results.json"
SCRIPT_DIR = ROOT / ".agents/ledger-bisect-2026-11-11"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cluster", required=True, help="cluster tag e.g. cluster_borrow")
    args = ap.parse_args()
    log = Path(f"/tmp/bisect_{args.cluster}.log")
    if not log.exists():
        raise SystemExit(f"missing {log}")
    text = log.read_text()
    m_bad = re.search(r"^FIRST_BAD=(\w+)$", text, re.M)
    m_subj = re.search(r"^SUBJECT=(.+)$", text, re.M)
    m_pr = re.search(r"^PR=(\w+)$", text, re.M)
    m_good = re.search(r"^GOOD=(\w+)$", text, re.M) or re.search(
        r"^KNOWN_GOOD=(\w+)$", text, re.M
    )
    if not m_bad:
        raise SystemExit("log has no FIRST_BAD")
    commit = m_bad.group(1)
    pr = (m_pr.group(1) if m_pr else "").replace("none", "")
    mech = subprocess.check_output(
        ["bash", str(SCRIPT_DIR / "mechanism.sh"), commit],
        text=True,
    ).strip()
    data = json.loads(RESULTS.read_text())
    for c in data["clusters"]:
        if c["id"] == args.cluster:
            c["breaking_commit"] = commit
            c["pr"] = pr
            c["mechanism"] = mech
            c["status"] = "bisected"
            if m_good:
                c["good_commit"] = m_good.group(1)
            if m_subj:
                c["subject"] = m_subj.group(1).strip()
            break
    else:
        raise SystemExit(f"cluster {args.cluster} not in results json")
    RESULTS.write_text(json.dumps(data, indent=2) + "\n")
    subprocess.check_call(["python3", str(SCRIPT_DIR / "write_part20.py")])


if __name__ == "__main__":
    main()
