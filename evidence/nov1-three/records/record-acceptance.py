import hashlib,json,os,re,shutil,subprocess,sys,time
from pathlib import Path
root=Path.cwd();out=root/'build/swarm/nov1';generation=next((out/'producers').glob('gen-ea132c1fff8*'));runner=str(generation/'corpus_runner');manifest=json.loads((generation/'GENERATION').read_text());env=dict(os.environ,CARGO_PROFILE_DEV_OPT_LEVEL='1',RAYON_NUM_THREADS='2',OMEGA_CORPUS_JOBS='2')
sys.path.insert(0,str(root/'tools'));import corpus_records

def run(name,command,allowed=(0,),timeout=1800):
 start=time.monotonic();r=subprocess.run(command,capture_output=True,text=True,env=env,timeout=timeout);(out/(name+'.stdout')).write_text(r.stdout);(out/(name+'.stderr')).write_text(r.stderr);print(name,r.returncode,round(time.monotonic()-start,2),flush=True)
 if r.returncode not in allowed:print((r.stdout+r.stderr)[-5000:]);raise SystemExit(r.returncode)
 return r

def unchanged_producer():
 for path,identity in manifest['source_inputs'].items():
  if subprocess.check_output(['git','rev-parse','HEAD:'+path],text=True).strip()!=identity:raise SystemExit('changed producer input '+path)
 if subprocess.check_output(['git','status','--porcelain'],text=True).strip():raise SystemExit('record requires clean committed input')

def outside(text,selected):
 starts=list(re.finditer(r'^(?:pass|fail|run|pending)/\S+[^\S\n]',text,re.M));pieces=[text[:starts[0].start()]] if starts else [text]
 for i,match in enumerate(starts):
  end=starts[i+1].start() if i+1<len(starts) else len(text);block=text[match.start():end];key=block.split()[0]
  if key not in selected:pieces.append(block)
 return ''.join(pieces)

log=[]
for name,leg,golden,filter_file in [('source',[],'tests/omega/corpus_outcomes.txt','source-record-filter.txt'),('native',['--native'],'tests/omega/corpus_native_linux_x86_64.txt','both-record-filter.txt'),('both',['--both'],'tests/omega/corpus_both_linux_x86_64.txt','both-record-filter.txt')]:
 unchanged_producer();filters=(out/filter_file).read_text().strip();selected=set(filters.split(','));before=Path(golden).read_text();(out/('before-record-'+name+'.txt')).write_text(before);head=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()
 if name=='native':
  run('native-pre-record-diff',['python3','tools/corpus_gate.py','--runner',runner,*leg,'--filter',filters,'--jev','--write-moves',str(out/'native-pre-record-moves.json'),'--json',str(out/'native-pre-record-diff.json')],allowed=(0,1))
  diff=json.loads((out/'native-pre-record-diff.json').read_text())
  if diff['oracle_failures'] or diff['counts']['lost_successes'] or any(x['status']!='built' or x.get('facts',{}).get('exit')!='0' for x in diff['observations']):raise SystemExit('native movement requires engineering review')
 run('record-'+name,['python3','tools/corpus_gate.py','--runner',runner,*leg,'--filter',filters,'--record','--json',str(out/('record-'+name+'.json'))],allowed=(0,1))
 document=json.loads((out/('record-'+name+'.json')).read_text());observations=document['observations'];actual={x['fixture'] for x in observations}
 if actual!=selected or document['oracle_failures'] or any(x['status'] not in {'checked','rejected','built'} or x.get('expected_satisfied') is False for x in observations):raise SystemExit('recorded selection/oracle mismatch')
 after=Path(golden).read_text();(out/('after-record-'+name+'.txt')).write_text(after)
 if outside(before,selected)!=outside(after,selected):raise SystemExit('outside golden byte changed')
 delta=subprocess.check_output(['git','diff','--name-only'],text=True).splitlines()
 if delta!=[golden]:raise SystemExit('unexpected tracked delta '+repr(delta))
 subprocess.run(['git','add',golden],check=True);subprocess.run(['git','commit','-m','test: pin selected const '+name+' observations'],check=True)
 log.append({'leg':name,'observed_source':head,'producer_revision':manifest['revision'],'producer_inputs_unchanged':True,'golden':golden,'recorded':len(actual),'outside_bytes_preserved':True,'before_sha256':hashlib.sha256(before.encode()).hexdigest(),'after_sha256':hashlib.sha256(after.encode()).hexdigest(),'result_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip()});(out/'record-ledger.json').write_text(json.dumps(log,indent=2)+'\n')
print('Recorded40source and14native/both rows; every outside golden byte preserved.',flush=True)
