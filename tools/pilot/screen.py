import os, subprocess, sys, concurrent.futures as cf, json, time
om, lst, out = sys.argv[1:4]
dirs=[l.strip() for l in open(lst) if l.strip()]
def one(d):
    t=time.time()
    try:
        p=subprocess.run([om,"run","--engine","psi",d+"/build.omg"],stdin=subprocess.DEVNULL,capture_output=True,text=True,timeout=120)
        c,o=p.returncode,(p.stdout+p.stderr)[-600:]
    except subprocess.TimeoutExpired: c,o="timeout",""
    return {"dir":d,"exit":c,"s":round(time.time()-t,1),"out":o}
with open(out,"w") as f, cf.ThreadPoolExecutor(2) as ex:
    for r in ex.map(one,dirs): f.write(json.dumps(r)+"\n"); f.flush()
