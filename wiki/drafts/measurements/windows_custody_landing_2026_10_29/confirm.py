"""Independently confirm the Windows custody-landing screen."""
import importlib.util,json,os,re,statistics,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]
BASE=ROOT/'build/windows-custody-landing'
DATA=BASE/'capture/confirmation'; DATA.mkdir()
spec=importlib.util.spec_from_file_location('first',ROOT/'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first=importlib.util.module_from_spec(spec); spec.loader.exec_module(first)
first.EVIDENCE=DATA/'logs'; first.EVIDENCE.mkdir(); first.self_test()
seed=json.loads((BASE/'baseline.json').read_text(encoding='utf-8'))
producers={'baseline':seed['producer'],'candidate':json.loads((BASE/'candidate/producer.json').read_text(encoding='utf-8'))['producer']}
for producer in producers.values(): assert first.require_optimized(producer['binary'])['sha256']==producer['sha256']
assert not any(k.startswith(('MIMALLOC_','MI_')) for k in os.environ)
expected={r['subject']:r['artifacts'] for r in seed['observations']}
caches={}
for flavor in producers:
    p=Path(os.environ['LOCALAPPDATA'])/('OmegaPerf-custody-landing-confirm-'+uuid.uuid4().hex[:12]); p.mkdir(); assert not list(p.iterdir()); caches[flavor]=p
results={'producers':producers,'sources':seed['source_manifest'],'targets':['windows_x86_64'],'cache_paths':{f:str(p) for f,p in caches.items()},'seed':[],'multi':[],'single':[],'profiles':[],'exclusions':[],'protocol':'Four balanced ABBA/BAAB blocks per Windows-only subject; eight samples per flavor/subject. Ordinary-depth private user caches; first visits and profiles separate. Fixed ec727c main baseline versus integrated custody candidate; exact images and actual Windows execution on every compile. All complete compile observations retained. All valid observations retained,32 Rayon workers. Fresh process/output. Exact image bytes, native Windows exit0 and canonical target order.'}
def retain(): (DATA/'results.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
def compile_one(flavor,subject,label,mode='multi',profile=False):
    targets=seed['targets'] if mode=='multi' else ['windows_x86_64']
    output=DATA/label; output.mkdir()
    env=dict(os.environ,LOCALAPPDATA=str(caches[flavor]),OMEGA_LIBRARY_ROOT=str(ROOT/'source/library'),RAYON_NUM_THREADS='32'); env.pop('OMEGA_PHASE_PROFILE',None)
    if profile: env['OMEGA_PHASE_PROFILE']='1'
    command=[producers[flavor]['binary'],'--timings','--build-dir',str(output)]
    for target in targets: command+=['--target',target]
    command.append(str(ROOT/first.SUBJECTS[subject]))
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
        sha=first.digest(artifact); assert sha==expected[subject][target]['sha256'],(row,target,sha,expected[subject][target])
        artifacts[target]={'path':str(artifact),'sha256':sha,'bytes':artifact.stat().st_size}
        if target=='windows_x86_64':
            native,_=first.capture([str(artifact)],label+'-native',dict(os.environ)); assert native['exit']==0; row['native']=native
    row.update(flavor=flavor,subject=subject,mode=mode,artifacts=artifacts)
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
    block_orders=[['candidate','baseline','baseline','candidate'],['baseline','candidate','candidate','baseline']]*2
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
assert all(first.require_optimized(p['binary'])['sha256']==p['sha256'] for p in producers.values())
results['complete']=True; retain()
for mode in ('single',):
    for subject in first.SUBJECTS:
        med={f:statistics.median(r['wall_ms'] for r in results[mode] if r['flavor']==f and r['subject']==subject) for f in producers}
        print(mode,subject,med,'total',1-med['candidate']/med['baseline'],flush=True)
