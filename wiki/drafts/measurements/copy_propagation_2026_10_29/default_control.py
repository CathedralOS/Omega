"""Bound rollback overhead with ordinary-empty/off/on interleaved compiles."""
import importlib.util,json,os,re,statistics,subprocess,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4];BASE=ROOT/'build/copy-propagation-replay';DATA=BASE/'default-control';DATA.mkdir()
spec=importlib.util.spec_from_file_location('first',ROOT/'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py');first=importlib.util.module_from_spec(spec);spec.loader.exec_module(first);first.EVIDENCE=DATA/'logs';first.EVIDENCE.mkdir();first.self_test()
prior=json.loads((BASE/'comparison/results.json').read_text());assert prior['complete'];producer=prior['producers']['on'];assert first.require_optimized(producer['binary'])['sha256']==producer['sha256']
manifest=json.loads((ROOT/'build/windows-custody-landing/candidate/build.json').read_text())['source_manifest'];assert all(first.digest(ROOT/p)==h for p,h in manifest.items())
default=DATA/'ordinary-build';default.mkdir();original=(ROOT/first.SUBJECTS['stack32']).parent
for p in original.rglob('*.omg'):
 target=default/p.relative_to(original);target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(p.read_bytes())
assert 'Optimizations::enable' not in (default/'build.omg').read_text()
source_hashes={str(p.relative_to(ROOT)):first.digest(p) for tree in (default,BASE/'witnesses/stack32') for p in tree.rglob('*.omg')};original_hashes={str(p.relative_to(ROOT)):first.digest(p) for p in original.rglob('*.omg')}
caches={f:Path(os.environ['LOCALAPPDATA'])/('OmegaPerf-copy-default-'+uuid.uuid4().hex[:12]) for f in ('default','off','on')}
for p in caches.values():p.mkdir();assert not list(p.iterdir())
results={'producer':producer,'controller_revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'source_hashes':source_hashes,'original_source_hashes':original_hashes,'cache_paths':{f:str(p) for f,p in caches.items()},'seed':[],'timed':[],'profiles':[],'protocol':'Eight warm samples per flavor, four symmetric three-arm blocks. Default uses unchanged authored Build with no selection and no rollback flag. Off/on share the selected private Build; off subtracts exactly CopyPropagation. Same frozen compiler, O3 profile and witness/oracle bytes. Native execution every compile. First visits/profiles separate. Ordinary private caches and new outputs/processes, no OS flush,32Rayon. Direct default/on comparison bounds interpretation; never pool with other cohorts.'}
def retain():(DATA/'results.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
def compile_one(flavor,label,profile=False):
 output=DATA/label;output.mkdir();env=dict(os.environ,LOCALAPPDATA=str(caches[flavor]),OMEGA_LIBRARY_ROOT=str(ROOT/'source/library'),RAYON_NUM_THREADS='32');env.pop('OMEGA_PHASE_PROFILE',None);assert not any(k.startswith(('MIMALLOC_','MI_')) for k in env)
 if profile:env['OMEGA_PHASE_PROFILE']='1'
 command=[producer['binary'],'--timings','--target','windows_x86_64','--build-dir',str(output)]
 if flavor=='off':command+=['--disable-optimization','CopyPropagation']
 command.append(str((default if flavor=='default' else BASE/'witnesses/stack32')/'main.omg'))
 row,text=first.capture(command,label,env);artifact=first.compiled_artifact(row,text,output);expected=prior['expected_images'][('on' if flavor=='on' else 'off')+'-stack32'];assert row['artifact_sha256']==expected
 native,_=first.capture([str(artifact)],label+'-native',dict(os.environ));assert native['exit']==0;row.update(flavor=flavor,native=native)
 if flavor=='off':assert 'requested=[CopyPropagation];applied=[CopyPropagation];effective=[]' in ''.join(text.split())
 else:assert 'optimizer rollback:' not in text
 if profile:
  assert re.search(r'[1-9]\d* n\s+source.graph.loaded',text) and re.search(r'[1-9]\d* n\s+source.graph.prepared_hits',text)
  row['spans']={m[3]:(float(m[1]),int(m[2])) for line in text.splitlines() if (m:=re.match(r'\s*([0-9.]+) ms\s+(\d+) x\s+(.+)',line))}
 print(label,round(row['wall_ms'],1),'ms',flush=True);return row
for f in ('default','off','on'):results['seed'].append(compile_one(f,'seed-'+f));retain()
orders=[['default','off','on','on','off','default'],['off','on','default','default','on','off'],['on','default','off','off','default','on'],['on','off','default','default','off','on']]
for block,order in enumerate(orders):
 for ordinal,f in enumerate(order):
  row=compile_one(f,f'{block}-{ordinal}-{f}');row['block']=block;results['timed'].append(row);retain()
for f in ('default','off','on'):results['profiles'].append(compile_one(f,'profile-'+f,True));retain()
assert all(first.digest(ROOT/p)==h for p,h in source_hashes.items());assert all(first.digest(ROOT/p)==h for p,h in original_hashes.items());assert all(first.digest(ROOT/p)==h for p,h in manifest.items());assert first.require_optimized(producer['binary'])['sha256']==producer['sha256']
results['complete']=True;retain();outputs={str(p.relative_to(ROOT)):first.digest(p) for p in DATA.rglob('*') if p.is_file() and p.suffix in ('.json','.log','.omg')};print(json.dumps({'captured_outputs':outputs},sort_keys=True),flush=True)
