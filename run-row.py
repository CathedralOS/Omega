import concurrent.futures,hashlib,json,subprocess,sys
from pathlib import Path
cli=str(Path(sys.argv[1]).resolve());label=sys.argv[2];root=Path('samples/cli/facts/saved_observation_snapshot/symbolic');places=['local','field','parameter','call_result','match_join','const_name','other_state','other_machine','target_body']
def run(job):
 name,bad=job;p=root/name/('invalid/main.omg' if bad else 'main.omg');command=[cli,'--check',str(p)] if bad else [cli,'run','--both',str(p)];r=subprocess.run(command,input='',capture_output=True,text=True,timeout=180);row=dict(name=name,invalid=bad,source_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),command=command,exit=r.returncode,stdout=r.stdout,stderr=r.stderr);print(json.dumps(row),flush=True);return row
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:rows=list(pool.map(run,[(name,bad) for name in places for bad in [False,True]]))
Path('build/symbolic-join/row-'+label+'.json').write_text(json.dumps(dict(cli=cli,producer_sha256=hashlib.sha256(Path(cli).read_bytes()).hexdigest(),rows=rows),indent=2)+'\n')
