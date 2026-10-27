import concurrent.futures,hashlib,json,subprocess,sys
from pathlib import Path
cli=Path(sys.argv[1]).resolve();label=sys.argv[2];root=Path('samples/cli/facts/saved_case_predicate')
def run(job):
 place,bad=job;p=root/place/('invalid/main.omg' if bad else 'main.omg');cmd=[str(cli),'--check',str(p)] if bad else [str(cli),'run','--both',str(p)];r=subprocess.run(cmd,capture_output=True,text=True,timeout=180);d=dict(place=place,invalid=bad,command=cmd,source_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),exit=r.returncode,stdout=r.stdout,stderr=r.stderr);print(place,bad,r.returncode,r.stderr.strip(),flush=True);return d
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:rows=list(pool.map(run,[(p.name,bad) for p in root.iterdir() if p.is_dir() for bad in [False,True]]))
Path('build/saved-case/row-'+label+'.json').write_text(json.dumps(rows,indent=2)+'\n')
