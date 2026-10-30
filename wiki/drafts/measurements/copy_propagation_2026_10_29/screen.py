"""Causal on/off screen of the existing admitted CopyPropagation family."""
import hashlib,importlib.util,json,os,re,statistics,subprocess,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]; BASE=ROOT/'build/copy-propagation-replay'; DATA=BASE/'capture'; DATA.mkdir()
spec=importlib.util.spec_from_file_location('first',ROOT/'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py'); first=importlib.util.module_from_spec(spec);spec.loader.exec_module(first);first.EVIDENCE=DATA/'logs';first.EVIDENCE.mkdir();first.self_test()
producer=json.loads((ROOT/'build/windows-custody-landing/candidate/producer.json').read_text())['producer']; build=json.loads((ROOT/'build/windows-custody-landing/candidate/build.json').read_text()); inputs=build['source_manifest']
assert all(first.digest(ROOT/p)==h for p,h in inputs.items());assert first.require_optimized(producer['binary'])['sha256']==producer['sha256']
witness=BASE/'witnesses'; witness.mkdir(); sources={}; private={}
for subject,relative in first.SUBJECTS.items():
 source=(ROOT/relative).parent; dest=witness/subject;dest.mkdir()
 for p in source.rglob('*.omg'):
  target=dest/p.relative_to(source);target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(p.read_bytes());sources[str(p.relative_to(ROOT))]=first.digest(p)
 p=dest/'build.omg'; assert p.is_file(); text=p.read_text(); assert 'Optimizations::enable' not in text and text.rstrip().endswith('}');p.write_text(text.rstrip()[:-1]+'    Optimizations::enable(&mut builder.optimizations, Optimization::CopyPropagation);\n}\n',newline='\n');private[subject]=str(dest/'main.omg')
private_hashes={str(p.relative_to(ROOT)):first.digest(p) for p in witness.rglob('*.omg')}
caches={f:Path(os.environ['LOCALAPPDATA'])/('OmegaPerf-copy-propagation-'+uuid.uuid4().hex[:12]) for f in ('off','on')}
for p in caches.values():p.mkdir();assert not list(p.iterdir())
results={'producer':producer,'controller_revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'sources':sources,'private_sources':private_hashes,'cache_paths':{f:str(p) for f,p in caches.items()},'seed':[],'timed':[],'profiles':[],'expected_images':{},'complete':False,'decision':'unfinished','protocol':'Same frozen producer and private authored CopyPropagation Build; off subtracts exact family by CLI rollback. Authored program/helper bytes unchanged. Distinct caches, outputs, process accounting and actual native oracles. Within-flavor deterministic images; changed policy may legitimately alter native bytes. Four balanced ABBA/BAAB blocks, 8 trials per leg; first visits/profile/report separate. No default selection or source change.'}
def retain(): (DATA/'results.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
def compile_one(flavor,subject,label,profile=False,report=False):
 output=DATA/label;output.mkdir();env=dict(os.environ,LOCALAPPDATA=str(caches[flavor]),OMEGA_LIBRARY_ROOT=str(ROOT/'source/library'),RAYON_NUM_THREADS='32');env.pop('OMEGA_PHASE_PROFILE',None)
 assert not any(k.startswith(('MIMALLOC_','MI_')) for k in env)
 if profile:env['OMEGA_PHASE_PROFILE']='1'
 command=[producer['binary'],'--timings','--target','windows_x86_64','--build-dir',str(output)]
 if flavor=='off':command+=['--disable-optimization','CopyPropagation']
 if report:command+=['--report-file',str(output/'report.txt')]
 command.append(private[subject]);row,text=first.capture(command,label,env);row.update(flavor=flavor,subject=subject)
 if row['exit']!=0 or re.search(r'^\s*error(?:\[|:)',text,re.M):return row,text
 artifact=first.compiled_artifact(row,text,output);native,_=first.capture([str(artifact)],label+'-native',dict(os.environ));row['native']=native;assert native['exit']==0
 key=flavor+'-'+subject
 if key in results['expected_images']:assert row['artifact_sha256']==results['expected_images'][key]
 else:results['expected_images'][key]=row['artifact_sha256']
 if flavor=='off':assert 'requested=[CopyPropagation];applied=[CopyPropagation];effective=[]' in ''.join(text.split()),text
 if profile:
  assert re.search(r'[1-9]\d* n\s+source.graph.loaded',text) and re.search(r'[1-9]\d* n\s+source.graph.prepared_hits',text)
  row['spans']={m[3]:(float(m[1]),int(m[2])) for line in text.splitlines() if (m:=re.match(r'\s*([0-9.]+) ms\s+(\d+) x\s+(.+)',line))}
  row['counters']={m[2]:int(m[1]) for line in text.splitlines() if (m:=re.match(r'\s*(\d+) n\s+(.+)',line))}
 print(label,round(row['wall_ms'],1),'ms',flush=True);return row,text
retain()
for flavor in ('off','on'):
 row,text=compile_one(flavor,'stack32','seed-'+flavor,report=True);results['seed'].append(row);retain()
 if row['exit']!=0:
  results['decision']='rejected: admitted configured path refused before timing; not a speedup';break
else:
 for block,order in enumerate((('off','on','on','off'),('on','off','off','on'))*2):
  for ordinal,flavor in enumerate(order):
   row,text=compile_one(flavor,'stack32',f'timed-{block}-{ordinal}-{flavor}');assert row['exit']==0;row['block']=block;results['timed'].append(row);retain()
 for flavor in ('off','on'):
  row,text=compile_one(flavor,'stack32','profile-'+flavor,True);assert row['exit']==0;results['profiles'].append(row);retain()
 med={f:statistics.median(r['wall_ms'] for r in results['timed'] if r['flavor']==f) for f in ('off','on')};results['medians']=med;results['reduction']=1-med['on']/med['off'];results['decision']='promising: needs activation/quality/full comparison/confirmation' if results['reduction']>=.2 else 'screen below substantial-win target; retain evidence and continue'
assert all(first.digest(ROOT/p)==h for p,h in inputs.items());assert all(first.digest(ROOT/p)==h for p,h in sources.items());assert all(first.digest(ROOT/p)==h for p,h in private_hashes.items());assert first.require_optimized(producer['binary'])['sha256']==producer['sha256']
results['complete']=True;retain()
outputs={str(p.relative_to(ROOT)):first.digest(p) for tree in (DATA,witness) for p in tree.rglob('*') if p.is_file() and p.suffix in ('.json','.log','.txt','.omg')};assert outputs;print(json.dumps({'captured_outputs':outputs},sort_keys=True),flush=True)
