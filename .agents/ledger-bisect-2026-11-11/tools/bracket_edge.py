#!/usr/bin/env python3
"""Find isolated 0->1 edge on first-parent chain using verify_at_commit (commit-scoped target)."""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DIR = Path(__file__).resolve().parents[1]
TOOL = DIR / "tools" / "verify_at_commit.sh"
MEASURE = DIR / "work" / "pin_measure.txt"
PIN = "51e8c45c793007f211d0d9baee7de16767172444"


def ledger_only(sha: str) -> bool:
    d = subprocess.run(
        ["git", "diff", "--name-only", f"{sha}^1", sha],
        capture_output=True,
        text=True,
        cwd=ROOT,
    )
    files = [ln for ln in (d.stdout or "").splitlines() if ln.strip()]
    if not files:
        return False
    return all(
        f == "tests/omega/corpus_outcomes.txt" or f.endswith(".md") for f in files
    )


def classify(witness: str, sha: str) -> tuple[int, str]:
    subprocess.run(
        [str(DIR / "tools/pin_bisect_context.py"), witness, str(MEASURE)],
        cwd=ROOT,
        check=True,
        stdout=subprocess.DEVNULL,
    )
    out = subprocess.check_output(
        [str(TOOL), sha, witness, "1"],
        text=True,
        cwd=ROOT,
    )
    data = json.loads(out)
    return data["results"][0], data.get("first_diag", "")


def chain(good: str, pin: str = PIN) -> list[str]:
    rev = subprocess.check_output(
        ["git", "rev-list", "--first-parent", pin],
        text=True,
        cwd=ROOT,
    ).splitlines()
    good = subprocess.check_output(
        ["git", "rev-parse", good], text=True, cwd=ROOT
    ).strip()
    pin = subprocess.check_output(
        ["git", "rev-parse", pin], text=True, cwd=ROOT
    ).strip()
    rev.reverse()
    if good not in rev:
        raise SystemExit(f"good {good} not on first-parent chain to pin")
    i = rev.index(good)
    return rev[i:]


def main() -> int:
    good, witness = sys.argv[1], sys.argv[2]
    commits = chain(good)
    codes: dict[str, int] = {}
    skips: list[dict] = []
    for sha in commits:
        if ledger_only(sha):
            skips.append({"sha": sha, "reason": "ledger_only"})
            continue
        try:
            rc, diag = classify(witness, sha)
        except Exception as exc:  # noqa: BLE001
            skips.append({"sha": sha, "reason": f"verify_error:{exc}"})
            continue
        if rc == 125:
            skips.append({"sha": sha, "reason": "build_or_other"})
            continue
        codes[sha] = rc
    prev_sha = None
    for sha in commits:
        if sha not in codes:
            continue
        if prev_sha is not None and codes.get(prev_sha) == 0 and codes[sha] == 1:
            between = commits[commits.index(prev_sha) + 1 : commits.index(sha)]
            mid_skip = [s for s in between if s not in codes]
            out = {
                "edge_good": prev_sha,
                "edge_bad": sha,
                "edge_parent": subprocess.check_output(
                    ["git", "rev-parse", f"{sha}^1"], text=True, cwd=ROOT
                ).strip(),
                "skipped_between": mid_skip,
                "isolated": len(mid_skip) == 0,
                "codes_sample": codes,
                "skips": skips,
            }
            print(json.dumps(out, indent=2))
            return 0
        if codes[sha] in (0, 1):
            prev_sha = sha
    out = {
        "no_isolated_edge": True,
        "codes_sample": codes,
        "skips": skips,
        "message": "first-bad within range, not isolated",
    }
    print(json.dumps(out, indent=2))
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
