import os,subprocess,time
from pathlib import Path
out=Path('build/swarm/nov1');generation=next((out/'producers').glob('gen-ea132c1fff8*'));runner=str((generation/'corpus_runner').resolve());filters=(out/'golden-review-filter.txt').read_text().strip();env=dict(os.environ,CARGO_PROFILE_DEV_OPT_LEVEL='1',RAYON_NUM_THREADS='2',OMEGA_CORPUS_JOBS='2')
for name,leg in [('source',[]),('both',['--both'])]:
 start=time.monotonic();r=subprocess.run(['python3','tools/corpus_gate.py','--runner',runner,*leg,'--filter',filters,'--jev','--write-moves',str(out/('golden-'+name+'-moves.json')),'--json',str(out/('golden-'+name+'-diff.json'))],capture_output=True,text=True,env=env,timeout=1800);(out/('golden-'+name+'-diff.stdout')).write_text(r.stdout);(out/('golden-'+name+'-diff.stderr')).write_text(r.stderr);print(name,r.returncode,round(time.monotonic()-start,2),flush=True)
 if r.returncode not in (0,1,3):print((r.stdout+r.stderr)[-5000:]);raise SystemExit(r.returncode)
