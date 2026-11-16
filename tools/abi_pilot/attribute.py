#!/usr/bin/env python3
"""attribute.py RUNDIR TARGET TRIPLE: root-cause each mismatching signature by its first wrong value."""
import json, sys, re
sys.argv, args = sys.argv[:1], sys.argv[1:]
rundir, target, triple = args[:3]
from trace import functions, placements
from omega_out import omega_maps
sigs = json.load(open(f"{rundir}/sigs.json"))
mir = dict(functions(open(f"{rundir}/clang.{triple}.mir").read()))
om = omega_maps(f"{rundir}/omega.{target}.txt")
def first(m, d):
    b = d["leaves"][0][0]
    return m.get(b, "<none>")
def name(d): return d.get("name") or "{" + ",".join(d["fields"]) + "}"
causes = {}
for s in sigs:
    k = s["id"]; o = om[k]
    if o["status"] != "OK": continue
    pm, rm = placements(k, mir[k], s)
    rows = []
    for i, d in enumerate(s["params"]):
        c, w = first(pm[i], d), first(o["params"][i], d)
        bad = any(not (pm[i].get(b,"").startswith("ind(") and o["params"][i].get(b,"").startswith("ind(")) and pm[i].get(b) != o["params"][i].get(b)
                  for b in [lo + j for lo, sz, _ in d["leaves"] for j in range(sz)])
        if c.startswith("ind(") and w.startswith("ind("): c, w = c.split(")@")[0]+")", w.split(")@")[0]+")"
        rows.append((i, d, o["classes"][i], c, w, bad or c != w))
    res_bad = False
    if s["result"]:
        d = s["result"]
        res_bad = any(rm.get(b) != o["result"].get(b) and not (str(rm.get(b)).startswith("ind(") and str(o["result"].get(b)).startswith("ind("))
                      for b in [lo + j for lo, sz, _ in d["leaves"] for j in range(sz)])
    bad_rows = [r for r in rows if r[5]]
    if not bad_rows and not res_bad: continue
    if not bad_rows:
        cause = "result"
    else:
        i, d, cls, c, w, _ = bad_rows[0]
        earlier = rows[:i]
        isfloat = cls in ("Float",) or cls.startswith("hfa")
        stacked_hfa_before = [r for r in earlier if r[2].startswith("hfa") and r[3].startswith("sp+")]
        if target == "macos_arm64" and cls == "Integer" and d.get("align") == 16 and c.startswith("reg:x") and w.startswith("reg:x") and int(w[5]) == int(c[5]) + 1:
            cause = "Apple: no even-register rounding for 16-byte-aligned args"
        elif isfloat and c.startswith("sp+") and w.startswith("reg:v") and stacked_hfa_before:
            cause = "C.3: HFA spilled to stack must close the vector registers (NSRN=8)"
        elif c.startswith("sp+") and w.startswith("sp+") and any(r[2].startswith("hfa") and r[1]["size"] % 8 and r[3].startswith("sp+") for r in earlier) and target == "linux_arm64":
            cause = "C.3/C.5: stacked HFA size not rounded up to 8"
        elif target == "macos_arm64" and c.startswith("sp+") and w.startswith("sp+"):
            prev = [r for r in earlier if r[3].startswith("sp+")]
            if prev and (prev[-1][1]["size"] < 8 or prev[-1][1]["size"] % 8) or int(c[3:]) % 8:
                cause = "Apple: stack args packed at natural alignment"
            elif any(r[2].startswith("hfa") and r[1]["size"] % 8 and r[3].startswith("sp+") for r in earlier):
                cause = "C.3/C.5: stacked HFA size not rounded up to 8"
            else:
                cause = "other"
        else:
            cause = "other"
    causes.setdefault(cause, []).append(k)
    if cause == "other" or "-v" in args:
        print(f"SIG {k} [{cause}]")
        for i, d, cls, c, w, b in rows:
            print(f"   {'!' if b else ' '}p{i} {name(d):22} {cls:16} clang={c:16} omega={w}")
        if s["result"]: print(f"   {'!' if res_bad else ' '}ret {name(s['result'])}")
for c, ks in causes.items():
    print(f"CAUSE {target} {len(ks):3}  {c}  e.g. {ks[:4]}")
