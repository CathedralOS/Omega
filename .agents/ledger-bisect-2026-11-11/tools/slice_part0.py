#!/usr/bin/env python3
"""Build part-0 prior slice membership and PIN recluster plan."""
from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
REPORT_CMD = [
    "git",
    "-C",
    str(REPO),
    "show",
    "origin/cursor/ledger-regressions-report-d9c6:wiki/drafts/audits/ledger_regressions_2026-11-11.md",
]


def prior_clusters() -> list[dict]:
    text = subprocess.check_output(REPORT_CMD, text=True)
    out: list[dict] = []
    for m in re.finditer(
        r"^\| (\d+) \| ([^|]+) \| ([^|]+) \| `([^`]+)` \| `([0-9a-f]{40})` \|\nMembers: (.+)$",
        text,
        re.M,
    ):
        partial = re.findall(r"`([^`]+)`", m.group(6))
        more_m = re.search(r"\(\+(\d+) more\)", m.group(6))
        out.append(
            {
                "rows": int(m.group(1)),
                "kind": m.group(2).strip(),
                "diag": m.group(3).strip(),
                "witness": m.group(4),
                "breaking_prior": m.group(5),
                "members_partial": partial,
                "members_more": int(more_m.group(1)) if more_m else 0,
            }
        )
    out.sort(key=lambda c: (-c["rows"], c["diag"]))
    return out


def recover_idx0_members(regressed: list[str]) -> list[str]:
    """Recover (+67 more) for open_array const_implementation sweep cluster."""
    keys = (
        "open_array_const_implementation",
        "open_const_implementation",
        "open_record_const_implementation",
    )
    extra_fail = (
        "fail/facts/open_const_implementation_changed_statement_output",
        "fail/facts/open_const_implementation_private_record_package_rejected",
        "fail/facts/open_const_implementation_private_result_guarantee",
        "fail/facts/open_const_implementation_return_write_output",
        "fail/facts/open_const_implementation_statement_const_result_rejected",
        "fail/facts/open_const_implementation_statement_saved_output_rejected",
        "fail/facts/open_record_const_implementation_specialization_agreement_rejected",
    )
    found = sorted(
        {
            f
            for f in regressed
            if any(k in f for k in keys) or f in extra_fail
        }
    )
    return found


def part_slice(k: int = 0, n: int = 40) -> list[tuple[int, dict]]:
    return [(i, c) for i, c in enumerate(prior_clusters()) if i % n == k]


def main() -> int:
    reg = [
        ln.strip()
        for ln in (REPO / "tests/omega/corpus_outcomes.txt").read_text().splitlines()
        if False
    ]
    reg_path = Path("/tmp/ledger-audit/regressed_fixtures.txt")
    if not reg_path.exists():
        reg_path.write_text(
            subprocess.check_output(
                [
                    "git",
                    "-C",
                    str(REPO),
                    "show",
                    "origin/cursor/ledger-regressions-report-d9c6:wiki/drafts/audits/regressed_fixtures.txt",
                ],
                text=True,
            )
        )
    reg = [ln.strip() for ln in reg_path.read_text().splitlines() if ln.strip()]
    slice_info = []
    for idx, c in part_slice(0, 40):
        if idx == 0:
            members = recover_idx0_members(reg)
        else:
            members = c["members_partial"]
            if c["members_more"]:
                # Single-row clusters already complete; multi partial should match count.
                pass
        slice_info.append(
            {
                "prior_idx": idx,
                "prior": c,
                "members": members,
            }
        )
    out = Path("/tmp/ledger-audit/part0_slice.json")
    out.write_text(json.dumps(slice_info, indent=2))
    print(f"prior_clusters_in_slice={len(slice_info)}")
    print(f"idx0_members={len(slice_info[0]['members'])} (prior rows={slice_info[0]['prior']['rows']})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
