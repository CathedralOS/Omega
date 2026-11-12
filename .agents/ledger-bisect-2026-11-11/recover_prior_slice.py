#!/usr/bin/env python3
"""Recover prior report part-4 member rows (markdown 226 clusters)."""
from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

import sys

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "wiki/drafts/audits/tools"))
from classify_corpus import is_worse_than_ledger, norm_diag, parse_records  # noqa: E402

REPORT = "origin/cursor/ledger-regressions-report-d9c6"
PART_K = 4
PART_N = 40
MEASURE_741 = "741270df60f7eab218e879eb6771d2bc75e64c00"


def sh(*args: str) -> str:
    return subprocess.check_output(args, text=True, cwd=REPO)


def prior_clusters_from_markdown() -> list[dict]:
    text = sh("git", "show", f"{REPORT}:wiki/drafts/audits/ledger_regressions_2026-11-11.md")
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
                "prior_index": len(clusters),
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
    return clusters


def load_observed_741() -> dict:
    """Observed corpus at sweep-era measurement (archived batches + host settle)."""
    corpus = REPO / "wiki/drafts/audits/corpus"
    batches = [corpus / f"batch_{i}.txt" for i in range(8)] + [corpus / "host_settled.txt"]
    text = ""
    for p in batches:
        if p.exists():
            text += p.read_text()
    if not text.strip():
        text = sh("git", "show", f"{REPORT}:wiki/drafts/audits/corpus/all_observed.txt")
    return parse_records(text)


def recover_members(cluster: dict, obs: dict, golden: dict, regressed: set[str]) -> list[str]:
    members = set(cluster["members_explicit"])
    if cluster["members_truncated"] == 0 and cluster["members_explicit"]:
        return sorted(members)
    w = cluster["prior_witness"]
    wdiag = (obs.get(w, {}).get("diagnostics") or [""])[0]
    keys: set[str] = set()
    if wdiag:
        keys.add(norm_diag(wdiag))
        if wdiag.startswith("authored declaration typing could not extend"):
            keys.add("__authored_prefix__")
    for fix in cluster["members_explicit"]:
        d = (obs.get(fix, {}).get("diagnostics") or [""])[0]
        if d:
            keys.add(norm_diag(d))
    for fix in regressed:
        if fix not in obs:
            continue
        o = obs[fix]
        g = golden.get(fix)
        if not g:
            continue
        if not is_worse_than_ledger(g, o) and fix.split("/")[0] == "fail":
            if not (o["status"] in ("rejected", "incomplete") and o.get("expected_satisfied") is False):
                continue
        elif not is_worse_than_ledger(g, o):
            continue
        d = (o.get("diagnostics") or [""])[0]
        if not d:
            continue
        dn = norm_diag(d)
        if dn in keys or (
            "__authored_prefix__" in keys and d.startswith("authored declaration typing could not extend")
        ):
            members.add(fix)
    return sorted(members)


def main() -> int:
    clusters = prior_clusters_from_markdown()
    part = [clusters[i] for i in range(len(clusters)) if i % PART_N == PART_K]
    golden = parse_records((REPO / "tests/omega/corpus_outcomes.txt").read_text())
    obs = load_observed_741()
    regressed = {
        ln.strip()
        for ln in sh("git", "show", f"{REPORT}:wiki/drafts/audits/regressed_fixtures.txt").splitlines()
        if ln.strip()
    }
    out_clusters = []
    all_fixtures: set[str] = set()
    for c in part:
        rec = recover_members(c, obs, golden, regressed)
        c = {**c, "members_recovered": rec}
        out_clusters.append(c)
        all_fixtures.update(rec)
    payload = {
        "measured_prior": MEASURE_741,
        "prior_slice_indices": [i for i in range(len(clusters)) if i % PART_N == PART_K],
        "clusters": out_clusters,
        "fixtures": sorted(all_fixtures),
    }
    out = REPO / ".agents/ledger-bisect-2026-11-11/prior_slice4.json"
    out.write_text(json.dumps(payload, indent=2))
    print("part4 clusters", len(part), "fixtures", len(all_fixtures))
    for c in out_clusters:
        print(
            c["prior_rows"],
            len(c["members_recovered"]),
            c["members_truncated"],
            c["prior_witness"][:55],
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
