import json,sys,os,re,collections
d=json.load(sys.stdin); root=d["workspace_root"]
rel=lambda p: os.path.relpath(os.path.dirname(p),root).replace("omega-rust/","")
pk={p["name"]:rel(p["manifest_path"]) for p in d["packages"]}
users=collections.defaultdict(set)
for p in d["packages"]:
    r=rel(p["manifest_path"])
    m=re.search(r"omega/compilation/pipeline/(\d\d)",r)
    m2=re.search(r"omega/compilation/representations/([^/]+)",r)
    if m: who="O"+m.group(1)
    elif m2: who="rep:"+m2.group(1)
    elif "omega/compilation/backend/" in r: who="backend"
    else: who=r.split("/")[2] if r.count("/")>=2 else r
    for dep in p["dependencies"]:
        t=pk.get(dep["name"],"")
        if dep.get("kind") is None and "omega/compilation/backend/" in t:
            users[t].add(who)
tot=0
for t in sorted(pk.values()):
    if "omega/compilation/backend/" not in t: continue
    loc=sum(sum(1 for _ in open(os.path.join(dp,f),errors="ignore")) for dp,_,fs in os.walk(os.path.join(root,"omega-rust",t,"src")) for f in fs if f.endswith(".rs"))
    tot+=loc
    print(f"{t.split('backend/')[1]:45s} {loc:7d}  users: {sorted(users[t])}")
print("total", tot)
