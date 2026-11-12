#!/usr/bin/env python3
"""Cluster still-broken fixtures by normalized first diagnostic."""
from __future__ import annotations

import json
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from classify_corpus import classify_all, norm_diag  # noqa: E402


def main() -> int:
    repo = Path(__file__).resolve().parents[3]
    observed = Path(sys.argv[1])
    regressed = Path(sys.argv[2])
    out = Path(sys.argv[3])
    data = classify_all(observed, regressed)
    clusters: dict[str, list] = defaultdict(list)
    for fix, detail in data["broken_detail"].items():
        key = detail["diag_norm"] or "(empty)"
        clusters[key].append({"fixture": fix, **detail})
    ranked = sorted(
        (
            {
                "diag_norm": k,
                "diag_sample": v[0]["diag"][:200],
                "count": len(v),
                "members": sorted(x["fixture"] for x in v),
                "witness": max(v, key=lambda x: len(x["fixture"]))["fixture"],
            }
            for k, v in clusters.items()
        ),
        key=lambda c: (-c["count"], c["diag_norm"]),
    )
    payload = {"counts": data["counts"], "clusters": ranked}
    out.write_text(json.dumps(payload, indent=2))
    print(f"clusters={len(ranked)} broken={data['counts']['broken']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
