import hashlib,json,subprocess,sys,time
from pathlib import Path
root=Path.cwd();out=root/'build/source-resolution-remaining';rows=[]
for label in ['00-A','01-B']:
 p=out/'screen'/label/'build/omega-program.exe';start=time.perf_counter()
 r={'label':label,'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}
 try:
  run=subprocess.run([str(p)],capture_output=True,timeout=30)
  r.update(exit=run.returncode,stdout_hex=run.stdout.hex(),stderr_hex=run.stderr.hex())
 except subprocess.TimeoutExpired:r['timeout_seconds']=30
 r['wall_ms']=(time.perf_counter()-start)*1000;rows.append(r)
 (out/'native-replays.json').write_text(json.dumps(rows,indent=2))
 assert r.get('exit')==0,r
 print('Native replay',label,'passed',flush=True)
assert len({r['sha256'] for r in rows})==1
records=json.loads((out/'screen/runs.json').read_text());assert len(records)==1
runner=root/'wiki/drafts/measurements/spill_loop_2026_10_28/profile.py'
for index,variant in enumerate('ABBAAB'):
 if index==0:continue
 label=f'{index:02}-{variant}'+('-retry' if index==1 else '')
 producer=root/('build/source-resolution/candidate/omega.exe' if variant=='A' else 'build/source-resolution-remaining/diagnostic/omega.exe')
 with (out/'screen'/(label+'.log')).open('w',encoding='utf-8') as log:
  subprocess.run([sys.executable,str(runner),str(producer),str(out/'screen'/label)],stdout=log,stderr=subprocess.STDOUT,timeout=180,check=True)
 row=json.loads((out/'screen'/label/'result.json').read_text());row.update(index=index,variant=variant,directory=label);records.append(row)
 (out/'screen/runs.json').write_text(json.dumps(records,indent=2));print(index,variant,round(row['wall_ms'],1),'ms; native verified',flush=True)
assert len(records)==6 and len({r['artifact_sha256'] for r in records})==1
