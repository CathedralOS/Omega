#!/usr/bin/env python3
"""Rebuild prior-report clusters from batch corpus records at 741270 measurement."""
from __future__ import annotations

import json
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
TOOLS = REPO / ".agents/ledger-bisect-2026-11-11/tools"
sys.path.insert(0, str(TOOLS))
from classify_corpus import classify_all, norm_diag  # noqa: E402

BR = "origin/cursor/ledger-regressions-report-d9c6"


def git_show(path: str) -> str:
    return subprocess.check_output(["git", "-C", str(REPO), "show", f"{BR}:{path}"], text=True)


def main() -> int:
    k = int(sys.argv[1])
    n = int(sys.argv[2])
    merged = Path("/tmp/prior_observed_merged.txt")
    parts = []
    for i in range(8):
        try:
            parts.append(git_show(f"wiki/drafts/audits/corpus/batch_{i}.txt"))
        except subprocess.CalledProcessError:
            break
    try:
        parts.append(git_show("wiki/drafts/audits/corpus/host_settled.txt"))
    except subprocess.CalledProcessError:
        pass
    merged.write_text("\n".join(parts))
    regressed = Path("/tmp/regressed_fixtures.txt")
    regressed.write_text(git_show("wiki/drafts/audits/regressed_fixtures.txt"))
    data = classify_all(merged, regressed)
    clusters: dict[str, list] = defaultdict(list)
    for fix, detail in data["broken_detail"].items():
        key = detail["diag_norm"] or "(empty)"
        clusters[key].append({"fixture": fix, **detail})
    ranked = sorted(
        (
            {
                "diag_norm": key,
                "diag_sample": items[0]["diag"][:200],
                "count": len(items),
                "members": sorted(x["fixture"] for x in items),
                "witness": max(items, key=lambda x: len(x["fixture"]))["fixture"],
            }
            for key, items in clusters.items()
        ),
        key=lambda c: (-c["count"], c["diag_norm"]),
    )
    mine = [c for i, c in enumerate(ranked) if i % n == k]
    out = {"slice_k": k, "slice_n": n, "prior_clusters_total": len(ranked), "slice_clusters": len(mine), "clusters": mine}
    Path(f"/tmp/slice_{k}_prior.json").write_text(json.dumps(out, indent=2))
    all_members = sorted({m for c in mine for m in c["members"]})
    Path(f"/tmp/slice_{k}_members.txt").write_text("\n".join(all_members) + "\n")
    print(f"prior_clusters={len(ranked)} slice_clusters={len(mine)} members={len(all_members)}")
    for i, c in enumerate(mine):
        print(f"  [{i}] rows={c['count']} witness={c['witness']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
