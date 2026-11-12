#!/usr/bin/env python3
"""Part 4 ledger bisect workflow helpers."""
from __future__ import annotations

import re
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
TOOLS = Path(__file__).resolve().parent / "tools"
import sys

sys.path.insert(0, str(TOOLS))
from classify_corpus import is_worse_than_ledger, norm_diag, parse_records  # noqa: E402

REPORT_REF = "origin/cursor/ledger-regressions-report-d9c6"
PART_K = 4
PART_N = 40


def load_prior_clusters() -> list[dict]:
    text = subprocess.check_output(
        ["git", "show", f"{REPORT_REF}:wiki/drafts/audits/ledger_regressions_2026-11-11.md"],
        text=True,
        cwd=REPO,
    )
    row_re = re.compile(r"^\| (\d+) \| ([^|]+) \| ([^|]+) \| `([^`]+)` \| `([^`]+)` \|")
    members_re = re.compile(r"Members: (.+)")
    clusters: list[dict] = []
    lines = text.splitlines()
    for i, line in enumerate(lines):
        m = row_re.match(line)
        if not m:
            continue
        count, kind, diag, witness, brk = m.groups()
        members: list[str] = []
        extra = 0
        if i + 1 < len(lines) and (mm := members_re.match(lines[i + 1])):
            body = mm.group(1)
            for part in body.split():
                if part.startswith("`") and part.endswith("`"):
                    members.append(part[1:-1])
            trunc = re.search(r"\(\+(\d+) more\)", body)
            extra = int(trunc.group(1)) if trunc else 0
        clusters.append(
            {
                "prior_rows": int(count),
                "prior_kind": kind.strip(),
                "prior_diag_sample": diag.strip(),
                "prior_witness": witness,
                "prior_breaking": brk,
                "members_explicit": members,
                "members_truncated": extra,
            }
        )
    clusters.sort(key=lambda c: (-c["prior_rows"], c["prior_diag_sample"]))
    return [clusters[i] for i in range(len(clusters)) if i % PART_N == PART_K]


def expand_cluster_members(cluster: dict, obs741_path: Path) -> list[str]:
    members = set(cluster["members_explicit"])
    if not obs741_path.exists():
        return sorted(members)
    obs = parse_records(obs741_path.read_text())
    prefix = "authored declaration typing could not extend"
    if cluster["prior_diag_sample"].startswith(prefix) or prefix in cluster["prior_witness"]:
        golden = parse_records((REPO / "tests/omega/corpus_outcomes.txt").read_text())
        for fix, row in obs.items():
            d = (row.get("diagnostics") or [""])[0]
            if not d.startswith(prefix):
                continue
            g = golden.get(fix)
            if not g:
                continue
            if fix.split("/")[0] == "fail":
                members.add(fix)
            elif is_worse_than_ledger(g, row):
                members.add(fix)
    return sorted(members)


def prior_fixture_union(obs741_path: Path) -> list[str]:
    fixtures: set[str] = set()
    for cluster in load_prior_clusters():
        fixtures.update(cluster["members_explicit"])
        fixtures.update(expand_cluster_members(cluster, obs741_path))
    return sorted(fixtures)
