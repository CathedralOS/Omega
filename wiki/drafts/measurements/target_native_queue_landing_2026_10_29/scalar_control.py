"""Keep per-target scalar meaning and native-evaluation refusal under batch completion."""
import importlib.util,json,os,re,sys,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]; BASE=ROOT/'build/native-queue-landing'
flavor=sys.argv[1]; assert flavor in ('baseline','candidate')
CAPTURE=BASE if flavor=='baseline' else BASE/'capture-2'
DATA=CAPTURE/('scalar-'+flavor); DATA.mkdir()
spec=importlib.util.spec_from_file_location('first',ROOT/'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first=importlib.util.module_from_spec(spec); spec.loader.exec_module(first); first.EVIDENCE=DATA/'logs'; first.EVIDENCE.mkdir()
producer=json.loads((BASE/flavor/'producer.json').read_text(encoding='utf-8'))['producer']
assert first.require_optimized(producer['binary'])['sha256']==producer['sha256']
targets=['linux_arm64','linux_x86_64','macos_arm64','windows_x86_64']
cases={'native-condition':('tests/omega/pass/layouts/portable_layout_constant_composition_exit','tests/omega/fail/layouts/portable_layout_constant_composition')}
result={'producer':producer,'observations':[],'source_manifest':{str(p.relative_to(ROOT)):first.digest(p) for paths in cases.values() for directory in paths for p in (ROOT/directory).iterdir() if p.suffix in ('.omg','.txt')}}
expected=json.loads((BASE/'scalar-baseline.json').read_text(encoding='utf-8')) if flavor=='candidate' else None
def retain(): (CAPTURE/('scalar-'+flavor+'.json')).write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
for subject,(positive,negative) in cases.items():
    cache=Path(os.environ['LOCALAPPDATA'])/('OmegaPerf-scalar-'+uuid.uuid4().hex[:12]); cache.mkdir()
    env=dict(os.environ,LOCALAPPDATA=str(cache),OMEGA_LIBRARY_ROOT=str(ROOT/'source/library'),RAYON_NUM_THREADS='32',OMEGA_PHASE_PROFILE='1')
    for kind,directory in (('positive',positive),('negative',negative)):
        label=subject+'-'+kind; output=DATA/label; output.mkdir()
        command=[producer['binary'],'--timings','--build-dir',str(output)]
        for target in targets: command+=['--target',target]
        command.append(str(ROOT/directory/'main.omg'))
        row,text=first.capture(command,label,env); row.update(subject=subject,kind=kind,artifacts={}); result['observations'].append(row); retain()
        assert re.findall(r'^target (\w+):$',text,re.M)==targets
        if flavor=='candidate': assert re.search(r'\b4 n\s+native.scheduling.jobs\b',text)
        published=re.findall(r'^published native output to (.+)$',text,re.M)
        if kind=='positive':
            assert row['exit']==0 and len(published)==4 and not re.search(r'^\s*error(?:\[|:)',text,re.M)
            for path in published:
                artifact=Path(path.strip()); target=next(t for t in targets if t in artifact.parts); assert artifact.is_file() and artifact.stat().st_size>0
                sha=first.digest(artifact); row['artifacts'][target]={'path':str(artifact),'sha256':sha,'bytes':artifact.stat().st_size}
                if expected: assert sha==next(r for r in expected['observations'] if r['subject']==subject and r['kind']=='positive')['artifacts'][target]['sha256']
                if target=='windows_x86_64':
                    native,_=first.capture([str(artifact)],label+'-native',dict(os.environ)); assert native['exit']==0; row['native']=native
        else:
            fragments=(ROOT/directory/'expected.txt').read_text(encoding='utf-8').strip().splitlines(); assert fragments
            assert row['exit']==1 and not published and all(fragment in text for fragment in fragments),(row,text[-3000:])
            assert re.findall(r'^target (\w+) failed',text,re.M)==targets
        retain(); print(flavor,label,'passed',flush=True)
assert all(first.digest(ROOT/p)==h for p,h in result['source_manifest'].items())
result['complete']=True; retain()
