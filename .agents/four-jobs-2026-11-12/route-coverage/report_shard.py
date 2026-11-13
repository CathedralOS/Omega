#!/usr/bin/env python3
"""Print merge report statistics for shard-S TSV."""

from __future__ import annotations

import collections
import sys
from pathlib import Path

SHARD = int(sys.argv[1]) if len(sys.argv) > 1 else 4
TSV = Path(__file__).resolve().parent / f"shard-{SHARD}.tsv"


def main() -> None:
    lines = TSV.read_text(encoding="utf-8").splitlines()
    header = lines[0].split("\t")
    rows = [dict(zip(header, line.split("\t"))) for line in lines[1:] if line.strip()]
    by_pv = collections.Counter(r.get("program_verdict", r.get("route_verdict", "")) for r in rows)
    by_agree = collections.Counter(r["agreement"] for r in rows)
    vacuous_n = sum(1 for r in rows if r.get("vacuous_accept_roots", ""))
    vacuous_row_n = sum(1 for r in rows if r.get("vacuous_accept_roots", ""))

    errors = [
        r
        for r in rows
        if r.get("program_verdict", r.get("route_verdict")) == "error"
    ]
    by_err = collections.Counter(r.get("first_refusal", "") or "?" for r in errors)

    print("rows", len(rows))
    print("program_verdict", dict(by_pv))
    print(
        "sum",
        sum(by_pv.values()),
        "=",
        len(rows),
    )
    print("agree", by_agree["agree"], "disagree", by_agree["disagree"], "n-a", by_agree["n-a"])
    print("accepted", by_pv["accepted"], "rejected", by_pv["rejected"])
    print("vacuous_rows", vacuous_row_n, "vacuous_accept_nonempty", vacuous_n)

    sound = [
        r
        for r in rows
        if r.get("program_verdict", r.get("route_verdict")) == "accepted"
        and r.get("current_verdict") == "rejected"
    ]
    print("route_accept_current_reject", len(sound))
    for r in sound:
        print(
            r["fixture"],
            "roots=" + r.get("roots", ""),
            "vacuous=" + r.get("vacuous_accept_roots", ""),
        )

    print("error_causes")
    for cause, count in by_err.most_common():
        print(count, cause[:120])


if __name__ == "__main__":
    main()
