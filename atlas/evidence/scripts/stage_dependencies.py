import json,sys,os
d=json.load(sys.stdin); root=d["workspace_root"]
rel=lambda p: os.path.relpath(os.path.dirname(p),root)
stages={p["name"]:rel(p["manifest_path"]) for p in d["packages"] if "/pipeline/" in rel(p["manifest_path"])}
out=set()
for p in d["packages"]:
    r=rel(p["manifest_path"])
    if "/pipeline/" in r: continue
    for dep in p["dependencies"]:
        if dep["name"] in stages and dep.get("kind") is None:
            s=stages[dep["name"]]
            out.add((r, ("psi " if "/psi/" in s else "omega ")+s.split("/pipeline/")[1]))
for a,b in sorted(out): print(f"{a}  ->  {b}")
