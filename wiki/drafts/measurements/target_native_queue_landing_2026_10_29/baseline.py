"""Freeze valid four-target baseline images before any parallelism edit."""
import importlib.util,json,os,re,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]
DATA=ROOT/'build/native-queue-landing'
DATA.mkdir(exist_ok=True)
spec=importlib.util.spec_from_file_location('first',ROOT/'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first=importlib.util.module_from_spec(spec); spec.loader.exec_module(first)
first.EVIDENCE=DATA/'baseline-logs'; first.EVIDENCE.mkdir()
producer=json.loads((DATA/'baseline/producer.json').read_text(encoding='utf-8'))['producer']
assert first.require_optimized(producer['binary'])['sha256']==producer['sha256']
targets=['linux_arm64','linux_x86_64','macos_arm64','windows_x86_64']
records={'producer':producer,'targets':targets,'observations':[],'source_manifest':{str(p.relative_to(ROOT)):first.digest(p) for source in first.SUBJECTS.values() for p in (ROOT/source).parent.rglob('*.omg')}}
cache=Path(os.environ['LOCALAPPDATA'])/('OmegaPerf-target-baseline-'+uuid.uuid4().hex[:12]); cache.mkdir()
for subject,source in first.SUBJECTS.items():
    output=DATA/('baseline-'+subject); output.mkdir()
    env=dict(os.environ,LOCALAPPDATA=str(cache),OMEGA_LIBRARY_ROOT=str(ROOT/'source/library'),RAYON_NUM_THREADS='32',OMEGA_PHASE_PROFILE='1')
    assert not any(k.startswith(('MIMALLOC_', 'MI_')) for k in env)
    command=[producer['binary'],'--timings','--build-dir',str(output)]
    for target in targets: command+=['--target',target]
    command.append(str(ROOT/source))
    row,text=first.capture(command,'baseline-'+subject,env)
    row.update(subject=subject,command=command)
    (DATA/(subject+'.attempt.json')).write_text(json.dumps(row,indent=2)+'\n',encoding='utf-8')
    assert row['exit']==0 and not re.search(r'^\s*error(?:\[|:)',text,re.M),row
    paths=re.findall(r'^published native output to (.+)$',text,re.M)
    assert len(paths)==4,(paths,text[-3000:])
    row['artifacts']={}
    for path in paths:
        artifact=Path(path.strip()); assert artifact.is_file() and artifact.stat().st_size>0
        matches=[t for t in targets if t in artifact.parts]; assert len(matches)==1,(path,artifact.parts)
        target=matches[0]; assert target not in row['artifacts']
        row['artifacts'][target]={'path':str(artifact),'sha256':first.digest(artifact),'bytes':artifact.stat().st_size}
        if target=='windows_x86_64':
            native,_=first.capture([str(artifact)],'baseline-'+subject+'-native',dict(os.environ)); assert native['exit']==0; row['native']=native
    assert set(row['artifacts'])==set(targets)
    row['spans']=[(float(m[1]),int(m[2]),m[3]) for line in text.splitlines() if (m:=re.match(r'\s*([0-9.]+) ms\s+(\d+) x\s+(.+)',line))]
    records['observations'].append(row)
    (DATA/'baseline.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
    print('baseline',subject,round(row['wall_ms'],1),'ms',[(t,a['bytes']) for t,a in row['artifacts'].items()],flush=True)
assert all(first.digest(ROOT/p)==v for p,v in records['source_manifest'].items())
assert first.require_optimized(producer['binary'])['sha256']==producer['sha256']
records['complete']=True
(DATA/'baseline.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
