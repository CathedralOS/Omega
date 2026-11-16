#!/usr/bin/env python3
"""diff.py RUNDIR TARGET CLANG_TRIPLE -> prints per-signature verdicts + summary."""
import json, sys
from trace import functions, placements, Harness
from omega_out import omega_maps

rundir, target, triple = sys.argv[1:4]
sigs = json.load(open(f"{rundir}/sigs.json"))
mir = dict(functions(open(f"{rundir}/clang.{triple}.mir").read()))


def value_bytes(d):
    return [o + j for o, s, _ in d["leaves"] for j in range(s)]


def ind_norm(loc):
    # indirect copies: compare only where the pointer travels, not temp offsets
    if loc.startswith("ind("):
        return loc.split(")@")[0] + ")"
    return loc


om = omega_maps(f"{rundir}/omega.{target}.txt")
stats = {"ok": 0, "mismatch": 0, "refused": 0, "harness": 0}
for s in sigs:
    k = s["id"]
    o = om[k]
    if o["status"] == "REFUSED":
        stats["refused"] += 1
        print(f"SIG {k} REFUSED {o['why']}")
        continue
    try:
        pm, rm = placements(k, mir[k], s)
    except Harness as e:
        stats["harness"] += 1
        print(f"SIG {k} HARNESS {e}")
        continue
    diffs = []
    items = [(f"p{i}", d, pm[i], o["params"][i]) for i, d in enumerate(s["params"])]
    if s["result"]:
        items.append(("ret", s["result"], rm, o["result"] or {}))
    for name, d, cm, omap in items:
        bad = []
        for b in value_bytes(d):
            c = cm.get(b, "<none>")
            w = omap.get(b, "<none>")
            if c.startswith("ind(") or w.startswith("ind("):
                c, w = ind_norm(c), ind_norm(w)
            if c != w:
                bad.append((b, c, w))
        if bad:
            desc = d.get("name") or "{" + ",".join(d["fields"]) + "}"
            cls = o["classes"][int(name[1:])] if name.startswith("p") else ""
            diffs.append(f"  {name} {desc} {cls}: byte{bad[0][0]} clang={bad[0][1]} omega={bad[0][2]} (+{len(bad)-1})")
    if diffs:
        stats["mismatch"] += 1
        sigtxt = ", ".join(p.get("name") or "{" + ",".join(p["fields"]) + "}" for p in s["params"])
        r = s["result"]
        rtxt = (r.get("name") or "{" + ",".join(r["fields"]) + "}") if r else "void"
        print(f"SIG {k} MISMATCH ({sigtxt}) -> {rtxt}")
        print("\n".join(diffs))
    else:
        stats["ok"] += 1
print("SUMMARY", target, stats)
