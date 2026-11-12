#!/usr/bin/env python3
from __future__ import annotations

import json
from collections import defaultdict
from pathlib import Path

import sys

sys.path.insert(0, str(Path(__file__).parent / "tools"))
from classify_corpus import is_worse_than_ledger, norm_diag, parse_records  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
OBS = ROOT / ".agents/ledger-bisect-2026-11-11/pin_observed.txt"
OUT = ROOT / ".agents/ledger-bisect-2026-11-11/pin_clusters.json"
TSV = ROOT / ".agents/ledger-bisect-2026-11-11/part-4.tsv"


def pin_class(fixture: str, obs: dict, golden: dict, raw_line: str) -> str:
    o = obs[fixture]
    g = golden.get(fixture)
    tier = o["tier"]
    if g is None:
        if tier == "fail":
            return "still_broken" if o["status"] not in ("rejected", "incomplete") else "fixed_on_main"
        return (
            "still_broken"
            if o["status"] not in ("built", "checked") or o.get("expected_satisfied") is False
            else "fixed_on_main"
        )
    if is_worse_than_ledger(g, o):
        return "still_broken"
    if tier == "fail" and g["status"] in ("rejected", "incomplete"):
        if o["status"] in ("rejected", "incomplete") and o.get("expected_satisfied") is False:
            return "still_broken"
    if tier != "fail" and o.get("expected_satisfied") is False:
        return "still_broken"
    if "unexpected" in raw_line:
        return "still_broken"
    return "fixed_on_main"


def main() -> int:
    golden = parse_records((ROOT / "tests/omega/corpus_outcomes.txt").read_text())
    obs = parse_records(OBS.read_text())
    raw_lines = {l.split()[0]: l for l in OBS.read_text().splitlines() if l and not l.startswith("\t")}
    results = []
    for fix in sorted(obs):
        cls = pin_class(fix, obs, golden, raw_lines.get(fix, ""))
        d = (obs[fix]["diagnostics"] or [""])[0]
        results.append({"fixture": fix, "class": cls, "diag": d})
    broken = [r for r in results if r["class"] == "still_broken"]
    fixed = [r for r in results if r["class"] == "fixed_on_main"]
    by: dict[str, list] = defaultdict(list)
    for r in broken:
        by[norm_diag(r["diag"])].append(r)
    clusters = []
    for i, (k, v) in enumerate(sorted(by.items(), key=lambda kv: (-len(kv[1]), kv[0]))):
        kind = "fail wrong reason" if v[0]["fixture"].startswith("fail/") else "pass/run rejected"
        witness = max(v, key=lambda x: len(x["fixture"]))["fixture"]
        clusters.append(
            {
                "cluster_id": i,
                "rows": len(v),
                "kind": kind,
                "diag_norm": k,
                "diag": v[0]["diag"],
                "witness": witness,
                "members": [x["fixture"] for x in v],
            }
        )
    OUT.write_text(json.dumps(clusters, indent=2))
    tsv_lines: list[str] = []
    fixture_cluster = {f: c["cluster_id"] for c in clusters for f in c["members"]}
    for r in results:
        cid = fixture_cluster.get(r["fixture"], "")
        tsv_lines.append(f"{r['fixture']}\t{r['class']}\t{r['diag']}\t{cid}\n")
    TSV.write_text("".join(tsv_lines))
    print(json.dumps({"fixed": len(fixed), "broken": len(broken), "clusters": len(clusters)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
