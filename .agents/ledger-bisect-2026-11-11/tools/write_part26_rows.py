#!/usr/bin/env python3
"""part-26.md with one attribution line per broken row (10 fixtures)."""
from __future__ import annotations

import json
from pathlib import Path

DIR = Path(__file__).resolve().parents[1]
PIN = "51e8c45c793007f211d0d9baee7de16767172444"


def load_verifies() -> dict[int, dict]:
    out = {}
    for p in sorted(DIR.glob("work/verify_c[0-9].json")):
        d = json.loads(p.read_text())
        out[d["cluster"]] = d
    return out


def main() -> int:
    clusters = json.loads((DIR / "work/pin_clusters.json").read_text())
    verifies = load_verifies()
    meta = json.loads((DIR / "work/part26_meta.json").read_text())
    fixed = set(meta.get("fixed", []))

    lines = [
        "# Ledger bisect part 26",
        "",
        f"Measured commit (PIN): `{PIN}`",
        "",
        "## Slice summary",
        "",
        "| still broken (rows attributed below) | 10 |",
        f"| fixed on main (negatives) | {len(fixed)} |",
        "",
        "## Per-row attribution (10 broken fixtures)",
        "",
        "| fixture | cluster | first-bad (40) | parent (40) | good anchor (40) | PR | edge verify parent×3 | edge verify bad×3 | mechanism |",
        "| --- | ---: | --- | --- | --- | ---: | --- | --- | --- |",
    ]

    tsv = ["fixture\tcluster\tfirst_bad\tparent\tgood_anchor\tpr\tparent_trials\tbad_trials\tmechanism_note"]

    fixture_to_cluster = {}
    for idx, c in enumerate(clusters):
        for m in c["members"]:
            fixture_to_cluster[m] = idx

    broken_fixtures = [m for m in meta["members"] if not m.startswith("fail/")]

    for fx in broken_fixtures:
        cidx = fixture_to_cluster[fx]
        v = verifies.get(cidx, {})
        bad = v.get("first_bad", "")
        parent = v.get("parent", "")
        good = v.get("good_anchor", "")
        pr = v.get("pr") or ""
        pt = v.get("parent_trials", {}).get("results", "")
        bt = v.get("bad_trials", {}).get("results", "")
        mech = "see work/verify_c{}.json".format(cidx)
        if v.get("mechanism_hunk_excerpt", "").startswith("diff"):
            mech = "quoted hunk in verify_c{}.json".format(cidx)
        if v.get("mechanism_hunk_excerpt") == "mechanism not established":
            mech = "mechanism not established"
        lines.append(
            f"| `{fx}` | {cidx} | `{bad}` | `{parent}` | `{good}` | {pr} | {pt} | {bt} | {mech} |"
        )
        tsv.append(f"{fx}\t{cidx}\t{bad}\t{parent}\t{good}\t{pr}\t{pt}\t{bt}\t{mech}")

    lines += ["", "## Cluster witnesses (summary)", ""]
    for idx, c in enumerate(clusters):
        v = verifies.get(idx, {})
        lines.append(f"### Cluster {idx} witness `{c['witness']}` ({c['count']} rows)")
        if v:
            lines.append(
                f"- parent trials {v.get('parent_trials',{}).get('results')}; "
                f"bad trials {v.get('bad_trials',{}).get('results')}"
            )
            br = (DIR / f"work/bracket_{c['witness'].split('/')[-1]}.json")
            if br.is_file():
                b = json.loads(br.read_text())
                if not b.get("isolated"):
                    lines.append(f"- **{b.get('verdict') or 'not isolated'}**; skipped: {b.get('skipped_between', [])[:5]}")

    for m in sorted(fixed):
        lines.append(f"- fixed on main: `{m}`")

    (DIR / "part-26.md").write_text("\n".join(lines) + "\n")
    (DIR / "part-26.tsv").write_text("\n".join(tsv) + "\n")
    print("wrote part-26 rows", len(broken_fixtures))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
