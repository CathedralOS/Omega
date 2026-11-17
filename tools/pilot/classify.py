import json, sys, re, collections
def cls(r):
    o = r["output"]
    if r["violations"]: return "VIOLATION"
    if "compile FAILED" in o: return "refused"
    if "execution FAILED" in o:
        m = re.search(r"cause: (\w+)", o)
        return "FAULT:" + (m.group(1) if m else "?")
    if r["exit"] == "timeout": return "timeout"
    if re.search(r"Psi (exit|crash)", o): return "ran"
    return "refused"
for path in sys.argv[1:]:
    rows = [json.loads(l) for l in open(path)]
    c = collections.Counter(cls(r) for r in rows)
    print(path, len(rows), dict(c))
    for r in rows:
        k = cls(r)
        if k.startswith(("FAULT", "VIOL", "timeout")):
            print("   ", k, "|", r["mutation"][:110], "|", r["mutant"].split("/")[-2:])
