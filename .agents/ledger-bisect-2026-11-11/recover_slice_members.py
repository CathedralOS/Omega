#!/usr/bin/env python3
"""Recover all members for report-slice prior clusters (index % N == k)."""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
BR = "origin/cursor/ledger-regressions-report-d9c6"
REPORT = Path("/tmp/ledger_report.md")


def git_show(path: str) -> str:
    return subprocess.check_output(["git", "-C", str(REPO), "show", f"{BR}:{path}"], text=True)


def parse_report_clusters(text: str) -> list[dict]:
    clusters: list[dict] = []
    lines = text.splitlines()
    row_re = re.compile(r"^\| (\d+) \| ([^|]+) \| (.+?) \| `([^`]+)` \|")
    i = 0
    while i < len(lines):
        m = row_re.match(lines[i])
        if m and "Breaking commit" not in lines[i]:
            rows, kind, diag, witness = m.groups()
            listed: list[str] = []
            more = 0
            if i + 1 < len(lines) and lines[i + 1].startswith("Members:"):
                ml = lines[i + 1]
                listed = re.findall(r"`([^`]+)`", ml)
                mm = re.search(r"\(\+(\d+) more\)", ml)
                if mm:
                    more = int(mm.group(1))
            clusters.append(
                {
                    "rows": int(rows),
                    "kind": kind.strip(),
                    "diag_sample": diag.strip(),
                    "witness": witness.strip(),
                    "members_listed": listed,
                    "members_more": more,
                }
            )
        i += 1
    return clusters


def expand_members(c: dict, regressed: list[str], prior_observed: Path) -> list[str]:
    sys.path.insert(0, str(REPO / ".agents/ledger-bisect-2026-11-11/tools"))
    from classify_corpus import parse_records, norm_diag  # noqa: E402

    obs = parse_records(prior_observed.read_text())
    members = set(c["members_listed"]) | {c["witness"]}
    if c["members_more"] > 0 and c["members_listed"]:
        sample = c["members_listed"][0]
        if "_through_" in sample:
            stem = sample.split("_through_")[0]
            for f in regressed:
                if stem in f:
                    members.add(f)
        # Diagnostic family from witness at prior measure
        wdiag = (obs.get(c["witness"], {}).get("diagnostics") or [""])[0]
        if wdiag and "partial write enclosure" in wdiag:
            for f in regressed:
                if any(
                    s in f
                    for s in (
                        "descriptor_range_exclusive_loan",
                        "match_loan_last_use",
                        "runtime_reassigned_view",
                    )
                ):
                    members.add(f)
            for f, rec in obs.items():
                d0 = (rec.get("diagnostics") or [""])[0]
                if "partial write enclosure" in d0 and f in regressed:
                    members.add(f)
        elif wdiag:
            wnorm = norm_diag(wdiag)
            for f, rec in obs.items():
                d0 = (rec.get("diagnostics") or [""])[0]
                if d0 and norm_diag(d0) == wnorm and f in regressed:
                    members.add(f)
    return sorted(members)


def main() -> int:
    k, n = int(sys.argv[1]), int(sys.argv[2])
    parts = []
    for i in range(8):
        try:
            parts.append(git_show(f"wiki/drafts/audits/corpus/batch_{i}.txt"))
        except subprocess.CalledProcessError:
            pass
    try:
        parts.append(git_show("wiki/drafts/audits/corpus/host_settled.txt"))
    except subprocess.CalledProcessError:
        pass
    Path("/tmp/all_prior_observed.txt").write_text("\n".join(parts))
    regressed = git_show("wiki/drafts/audits/regressed_fixtures.txt").splitlines()
    ranked = sorted(
        parse_report_clusters(REPORT.read_text()),
        key=lambda c: (-c["rows"], c["diag_sample"]),
    )
    prior_tables = [c for i, c in enumerate(ranked) if i % n == k]
    out = []
    all_members: set[str] = set()
    for c in prior_tables:
        mem = expand_members(c, regressed, Path("/tmp/all_prior_observed.txt"))
        all_members.update(mem)
        out.append(
            {
                "prior_index": ranked.index(c),
                **c,
                "members_recovered": mem,
                "recovered_count": len(mem),
            }
        )
    payload = {"k": k, "n": n, "prior_tables": out, "all_members": sorted(all_members)}
    Path(f"/tmp/slice_{k}_prior_tables.json").write_text(json.dumps(payload, indent=2))
    Path(f"/tmp/slice_{k}_members.txt").write_text("\n".join(sorted(all_members)) + "\n")
    print(f"prior_tables={len(out)} members={len(all_members)}")
    for t in out:
        print(f"  idx={t['prior_index']} report_rows={t['rows']} recovered={t['recovered_count']} witness={t['witness']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
