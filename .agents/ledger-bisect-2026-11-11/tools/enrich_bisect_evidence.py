#!/usr/bin/env python3
"""Attach first-bad evidence: stability, member corpus rows, diff hunk."""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

MAIN = Path(__file__).resolve().parents[1]
ROOT = Path(os.environ.get("BISECT_REPO_ROOT", MAIN.parents[1]))
PREPARE = MAIN / "tools/prepare_bisect_expect.py"
SCRIPT = MAIN / "tools/bisect_witness.sh"
PIN = "51e8c45c793007f211d0d9baee7de16767172444"
REC = Path("/tmp/bisect_one_record.txt")


def prepare(witness: str) -> str:
    env = {**os.environ, "BISECT_PIN_LEDGER": os.environ.get("BISECT_PIN_LEDGER", "/tmp/bisect_pin_corpus_outcomes.txt")}
    out = subprocess.check_output(["python3", str(PREPARE), witness], env=env, text=True).strip()
    return out


def run_witness(witness: str, expect_file: str) -> int:
    env = {
        **os.environ,
        "BISECT_WITNESS": witness,
        "BISECT_REPO_ROOT": str(ROOT),
        "BISECT_EXPECT_FILE": expect_file,
    }
    return subprocess.call(["nice", "-n", "19", str(SCRIPT)], cwd=ROOT, env=env)


def member_line(witness: str) -> str | None:
    if not REC.exists():
        return None
    for line in REC.read_text().splitlines():
        if line.startswith(witness + " "):
            return line
    return None


def triplet_witness(witness: str) -> list[int]:
    ef = prepare(witness)
    return [run_witness(witness, ef) for _ in range(3)]


def record_members(members: list[str]) -> dict[str, str | None]:
    out: dict[str, str | None] = {}
    for m in members:
        ef = prepare(m)
        run_witness(m, ef)
        out[m] = member_line(m)
    return out


def diff_hunk(sha: str) -> str:
    files = subprocess.check_output(
        ["git", "diff-tree", "--no-commit-id", "--name-only", "-r", sha],
        cwd=ROOT,
        text=True,
    ).splitlines()
    code = [f for f in files if f != "tests/omega/corpus_outcomes.txt" and not f.endswith(".md")]
    if not code:
        return "mechanism not established"
    patch = subprocess.check_output(
        ["git", "show", sha, "--format=", "-U3", "--", code[0]],
        cwd=ROOT,
        text=True,
        errors="replace",
    )
    lines = patch.splitlines()[:28]
    return "\n".join(lines) if lines else "mechanism not established"


def pr_number(sha: str) -> str | None:
    subj = subprocess.check_output(["git", "log", "-1", "--format=%s", sha], cwd=ROOT, text=True).strip()
    m = re.search(r"#(\d+)\b", subj)
    return m.group(1) if m else None


def skip_summary(step_log: Path, good: str, pin: str) -> dict:
    if not step_log.exists():
        return {}
    try:
        rev = subprocess.check_output(
            ["git", "rev-list", "--first-parent", f"{good}..{pin}"],
            cwd=ROOT,
            text=True,
        ).splitlines()
    except subprocess.CalledProcessError:
        rev = []
    rev_set = set(rev) | {good, pin}
    counts: dict[str, int] = {}
    samples: list[str] = []
    for line in step_log.read_text().splitlines():
        parts = line.split("\t")
        if len(parts) < 4:
            continue
        sha, code, kind = parts[0], parts[1], parts[2]
        if sha not in rev_set:
            continue
        if code == "125":
            counts[kind] = counts.get(kind, 0) + 1
            if len(samples) < 8:
                samples.append(f"{sha}:{kind}:{parts[3][:40] if len(parts)>3 else ''}")
    return {
        "range_first_parent": f"{good}..{pin}",
        "skipped_reason_counts": counts,
        "sample_skipped": samples,
    }


def main() -> int:
    cluster_json = Path(sys.argv[1])
    payload = json.loads(cluster_json.read_text())
    witness = payload["witness"]
    good = payload.get("good_commit") or ""
    brk = payload.get("breaking_commit") or ""
    clusters = json.loads((MAIN / "pin_clusters.json").read_text())
    members = next((c["members"] for c in clusters if c["witness"] == witness), [witness])

    if payload.get("mechanism", "").startswith("never-good since"):
        payload["fixture_added"] = payload.get("fixture_added") or payload["mechanism"].split()[-1]
        cluster_json.write_text(json.dumps(payload, indent=2))
        return 0

    if not brk or len(brk) != 40 or brk == PIN or payload.get("status") != "bisected":
        cluster_json.write_text(json.dumps(payload, indent=2))
        return 0

    parent = subprocess.check_output(["git", "rev-parse", f"{brk}^1"], cwd=ROOT, text=True).strip()

    subprocess.run(["git", "checkout", "-q", brk], cwd=ROOT, check=True)
    payload["breaking_exit_triplet"] = triplet_witness(witness)
    payload["members_at_breaking"] = record_members(members)

    subprocess.run(["git", "checkout", "-q", parent], cwd=ROOT, check=True)
    payload["parent_sha"] = parent
    payload["parent_exit_triplet"] = triplet_witness(witness)
    payload["members_at_parent"] = record_members(members)

    payload["breaking_commit"] = brk
    payload["good_commit"] = good
    payload["pr"] = pr_number(brk)
    payload["diff_hunk"] = diff_hunk(brk)
    step_log = Path(os.environ.get("BISECT_STEP_LOG", str(MAIN / "bisect_steps.tsv")))
    if payload.get("used_linear_fallback"):
        payload["skip_fallback"] = skip_summary(step_log, good, PIN)
        if payload.get("bisect_skip_note"):
            payload["skip_fallback"]["bisect_note"] = payload["bisect_skip_note"]

    subprocess.run(["git", "checkout", "-q", PIN], cwd=ROOT, check=False)
    cluster_json.write_text(json.dumps(payload, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
