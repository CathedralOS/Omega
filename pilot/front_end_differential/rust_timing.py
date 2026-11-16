import json,os,subprocess,sys,tempfile,time
from concurrent.futures import ThreadPoolExecutor
CLI=sys.argv[1]; rows=[json.loads(l) for l in open(sys.argv[2])]
def one(r):
    with tempfile.TemporaryDirectory() as d:
        p=os.path.join(d,"main.omg"); open(p,"w").write(r["source"]); t=time.time()
        try: c=subprocess.run([CLI,"--check",p],stdin=subprocess.DEVNULL,capture_output=True,timeout=120,cwd=d).returncode
        except subprocess.TimeoutExpired: c="timeout"
        return r["mutation"],c,time.time()-t
t=time.time()
with ThreadPoolExecutor(4) as ex: res=list(ex.map(one,rows))
import collections
print("wall",round(time.time()-t,1),"s for",len(res)); ts=sorted(x[2] for x in res); print("median",round(ts[len(ts)//2],2),"p95",round(ts[int(len(ts)*.95)],2),"max",round(ts[-1],2))
print(collections.Counter((m, "accept" if c==0 else "refuse") for m,c,_ in res))
