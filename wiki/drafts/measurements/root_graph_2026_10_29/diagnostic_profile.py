from pathlib import Path
import importlib.util
import json
import os
import re

ROOT=Path.cwd()
DATA=ROOT/'build/root-graph-policy'
saved=json.loads((DATA/'results.json').read_text(encoding='utf-8'))
assert saved['complete']
producer=saved['plan']['producer']
record_relative=Path(saved['plan']['record_relative'])
spec=importlib.util.spec_from_file_location('first',ROOT/'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first=importlib.util.module_from_spec(spec)
spec.loader.exec_module(first)
first.EVIDENCE=DATA/'logs'
rows=[]
for policy in ('shared','scoped'):
 for subject,relative in first.SUBJECTS.items():
  cache=DATA/(policy+'-cache')
  record=cache/record_relative
  slot=DATA/'root-records'/(subject+'.rec')
  if policy=='scoped':
   assert slot.resolve().is_relative_to(DATA.resolve()) and not record.exists()
   slot.replace(record)
  label='profile-'+policy+'-'+subject
  output=DATA/label
  output.mkdir()
  env=dict(os.environ,LOCALAPPDATA=str(cache),OMEGA_LIBRARY_ROOT=str(ROOT/'source/library'),RAYON_NUM_THREADS='32',OMEGA_PHASE_PROFILE='1')
  row,text=first.capture([producer['binary'],'--timings','--target','windows_x86_64','--build-dir',str(output),str(ROOT/relative)],label,env)
  artifact=first.compiled_artifact(row,text,output)
  expected=next(x['artifact_sha256'] for x in saved['warm'] if x['subject']==subject)
  assert row['artifact_sha256']==expected
  row.update(policy=policy,subject=subject,cache_loaded=bool(re.search(r'[1-9]\d* n\s+source.graph.loaded',text)),prepared_hit=bool(re.search(r'[1-9]\d* n\s+source.graph.prepared_hits',text)))
  assert row['cache_loaded'] and row['prepared_hit'],row
  row['spans']=[(float(m[1]),int(m[2]),m[3]) for line in text.splitlines() if (m:=re.match(r'\s*([0-9.]+) ms\s+(\d+) x\s+(.+)',line))]
  if policy=='scoped':record.replace(slot)
  execution,_=first.capture([str(artifact)],label+'-native',dict(os.environ))
  row['native_exit']=execution['exit']
  assert execution['exit']==0
  rows.append(row)
  (DATA/'profiles.json').write_text(json.dumps(rows,indent=2)+'\n',encoding='utf-8')
  print(label,[(ms,name) for ms,n,name in row['spans'] if name.startswith('source.graph.load.')],flush=True)
