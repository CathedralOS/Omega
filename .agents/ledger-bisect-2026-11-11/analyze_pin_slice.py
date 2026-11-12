#!/usr/bin/env python3
"""Classify PIN re-measurement and emit clusters for bisect."""
from __future__ import annotations

import json
import sys
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
TOOLS = REPO / ".agents/ledger-bisect-2026-11-11/tools"
sys.path.insert(0, str(TOOLS))
from classify_corpus import classify_all, norm_diag  # noqa: E402


def main() -> int:
    k = int(sys.argv[1])
    observed = Path(f"/tmp/slice_{k}_pin_measure.txt")
    regressed = Path(f"/tmp/slice_{k}_members.txt")
    data = classify_all(observed, regressed)
    clusters: dict[str, list] = defaultdict(list)
    for fix, detail in data["broken_detail"].items():
        key = detail["diag_norm"] or "(empty)"
        clusters[key].append({"fixture": fix, **detail})
    ranked = sorted(
        (
            {
                "diag_norm": key,
                "diag_sample": items[0]["diag"][:240],
                "count": len(items),
                "members": sorted(x["fixture"] for x in items),
                "witness": max(items, key=lambda x: len(x["fixture"]))["fixture"],
            }
            for key, items in clusters.items()
        ),
        key=lambda c: (-c["count"], c["diag_norm"]),
    )
    tsv_lines = []
    for ci, c in enumerate(ranked):
        for m in c["members"]:
            d = next(x for x in clusters[c["diag_norm"]] if x["fixture"] == m)
            tsv_lines.append(f"{m}\tstill broken\t{d['diag'][:200]}\t{k}-cluster-{ci}")
    for f in data["fixed"]:
        tsv_lines.append(f"{f}\tfixed on main\t\t")
    out = {
        "pin": "51e8c45c793007f211d0d9baee7de16767172444",
        "counts": data["counts"],
        "fixed": sorted(data["fixed"]),
        "clusters": ranked,
    }
    Path(f"/tmp/slice_{k}_pin_analysis.json").write_text(json.dumps(out, indent=2))
    Path(f"/tmp/slice_{k}_tsv_body").write_text("\n".join(tsv_lines) + "\n")
    print(json.dumps({"fixed": data["counts"]["fixed"], "broken": data["counts"]["broken"], "clusters": len(ranked)}))
    for c in ranked:
        print(f"  cluster rows={c['count']} witness={c['witness']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
