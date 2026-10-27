import concurrent.futures,hashlib,json,subprocess,sys
from pathlib import Path
cli=Path(sys.argv[1]).resolve();label=sys.argv[2];both=len(sys.argv)>3 and sys.argv[3]=='both';root=Path('samples/cli/facts/saved_case_predicate');places=['local','field','parameter','call_result','match_join','const_name','other_state','other_machine','target_body','writes_before_read']
def run(job):
 place,bad=job;p=root/place/('invalid/main.omg' if bad else 'main.omg');command=[str(cli),'run','--both',str(p)] if both and not bad else [str(cli),'--check',str(p)];r=subprocess.run(command,input='',capture_output=True,text=True,timeout=180);row=dict(place=place,invalid=bad,command=command,source_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),exit=r.returncode,stdout=r.stdout,stderr=r.stderr);print(place,bad,r.returncode,r.stderr.strip(),flush=True);return row
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:rows=list(pool.map(run,[(place,bad) for place in places for bad in [False,True]]))
Path('build/saved-case-calls/row-'+label+'.json').write_text(json.dumps(rows,indent=2)+'\n')
