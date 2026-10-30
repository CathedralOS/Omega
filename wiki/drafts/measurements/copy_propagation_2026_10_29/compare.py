"""Screen Windows sibling verification with pinned images and native oracles."""
import importlib.util,json,os,re,statistics,uuid,sys,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]
BASE=ROOT/'build/copy-propagation-replay'
COHORT=sys.argv[1]; assert COHORT in ('comparison','confirmation')
DATA=BASE/COHORT; DATA.mkdir()
spec=importlib.util.spec_from_file_location('first',ROOT/'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first=importlib.util.module_from_spec(spec); spec.loader.exec_module(first)
first.EVIDENCE=DATA/'logs'; first.EVIDENCE.mkdir(); first.self_test()
seed=json.loads((BASE/'capture/results.json').read_text(encoding='utf-8')); assert seed['complete']
producer=seed['producer']; producers={'off':producer,'on':producer}
assert first.require_optimized(producer['binary'])['sha256']==producer['sha256']
manifest=json.loads((ROOT/'build/windows-custody-landing/candidate/build.json').read_text())['source_manifest']
assert all(first.digest(ROOT/p)==h for p,h in manifest.items())
assert all(first.digest(ROOT/p)==h for p,h in seed['private_sources'].items())
expected=dict(seed['expected_images'])
if COHORT=='confirmation':
 previous=json.loads((BASE/'comparison/results.json').read_text()); assert previous['complete']; expected.update(previous['expected_images'])
caches={}
for flavor in producers:
    p=Path(os.environ['LOCALAPPDATA'])/('OmegaPerf-copy-prop-'+COHORT+'-'+uuid.uuid4().hex[:12]); p.mkdir(); assert not list(p.iterdir()); caches[flavor]=p
results={'producers':producers,'sources':seed['sources'],'private_sources':seed['private_sources'],'expected_images':expected,'controller_revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'targets':['windows_x86_64'],'cache_paths':{f:str(p) for f,p in caches.items()},'seed':[],'multi':[],'single':[],'profiles':[],'exclusions':[],'protocol':'Four balanced ABBA/BAAB blocks per Windows-only subject; eight samples per flavor/subject. Ordinary-depth private user caches; first visits and profiles separate. Same frozen O3 producer and same private authored CopyPropagation Build. Off uses exact family rollback; on retains authored selection. Image determinism within each flavor; actual native execution every compile. All complete compile observations retained. All valid observations retained,32 Rayon workers. Fresh process/output. Exact image bytes, native Windows exit0 and canonical target order.'}
def retain(): (DATA/'results.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
def compile_one(flavor,subject,label,mode='multi',profile=False):
    targets=['windows_x86_64']
    output=DATA/label; output.mkdir()
    env=dict(os.environ,LOCALAPPDATA=str(caches[flavor]),OMEGA_LIBRARY_ROOT=str(ROOT/'source/library'),RAYON_NUM_THREADS='32'); env.pop('OMEGA_PHASE_PROFILE',None)
    if profile: env['OMEGA_PHASE_PROFILE']='1'
    command=[producers[flavor]['binary'],'--timings','--build-dir',str(output)]
    for target in targets: command+=['--target',target]
    if flavor=='off': command+=['--disable-optimization','CopyPropagation']
    command.append(str(BASE/'witnesses'/subject/'main.omg'))
    row,text=first.capture(command,label,env)
    (DATA/(label+'.compile.json')).write_text(json.dumps(row,indent=2),encoding='utf-8')
    assert row['exit']==0 and not re.search(r'^\s*error(?:\[|:)',text,re.M),row
    paths=re.findall(r'^published native output to (.+)$',text,re.M); assert len(paths)==len(targets),(row,paths)
    if mode=='multi': assert re.findall(r'^target (\w+):$',text,re.M)==targets
    artifacts={}
    for path in paths:
        artifact=Path(path.strip()); assert artifact.is_file() and artifact.stat().st_size>0
        target=next(t for t in targets if t in artifact.parts) if mode=='multi' else targets[0]
        assert target not in artifacts
        sha=first.digest(artifact); key=flavor+'-'+subject
        if key in expected: assert sha==expected[key],(row,target,sha,expected[key])
        else: expected[key]=sha
        artifacts[target]={'path':str(artifact),'sha256':sha,'bytes':artifact.stat().st_size}
        if target=='windows_x86_64':
            native,_=first.capture([str(artifact)],label+'-native',dict(os.environ)); assert native['exit']==0; row['native']=native
    row.update(flavor=flavor,subject=subject,mode=mode,artifacts=artifacts)
    if flavor=='off': assert 'requested=[CopyPropagation];applied=[CopyPropagation];effective=[]' in ''.join(text.split())
    if profile:
        assert re.search(r'[1-9]\d* n\s+source.graph.loaded',text) and re.search(r'[1-9]\d* n\s+source.graph.prepared_hits',text)
        row['spans']=[(float(m[1]),int(m[2]),m[3]) for line in text.splitlines() if (m:=re.match(r'\s*([0-9.]+) ms\s+(\d+) x\s+(.+)',line))]
    print(label,round(row['wall_ms'],1),'ms',flush=True)
    return row
retain()
for mode in ('single',):
    for index,subject in enumerate(first.SUBJECTS):
        for flavor in list(producers)[index:]+list(producers)[:index]:
            results['seed'].append(compile_one(flavor,subject,f'{mode}-seed-{flavor}-{subject}',mode)); retain()
    block_orders=[['off','on','on','off'],['on','off','off','on']]*2
    if COHORT=='confirmation': block_orders.reverse()
    for block,order in enumerate(block_orders):
        for subject in first.SUBJECTS:
            for ordinal,flavor in enumerate(order):
                row=compile_one(flavor,subject,f'{mode}-{block}-{ordinal}-{flavor}-{subject}',mode); row['block']=block
                results[mode].append(row); retain()
    if mode=='single':
        for subject in first.SUBJECTS:
            for flavor in producers:
                results['profiles'].append(compile_one(flavor,subject,f'profile-{flavor}-{subject}',mode,True)); retain()
assert all(first.digest(ROOT/p)==v for p,v in results['sources'].items())
assert all(first.digest(ROOT/p)==v for p,v in seed['private_sources'].items())
assert all(first.digest(ROOT/p)==h for p,h in manifest.items())
assert all(first.require_optimized(p['binary'])['sha256']==p['sha256'] for p in producers.values())
results['complete']=True; retain()
for mode in ('single',):
    for subject in first.SUBJECTS:
        med={f:statistics.median(r['wall_ms'] for r in results[mode] if r['flavor']==f and r['subject']==subject) for f in producers}
        print(mode,subject,med,'total',1-med['on']/med['off'],flush=True)
