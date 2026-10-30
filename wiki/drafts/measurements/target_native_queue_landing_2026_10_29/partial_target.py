"""One refused target directory cannot lose admitted native siblings."""
import importlib.util,json,os,re,uuid
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4]
BASE=ROOT/'build/native-queue-landing'
DATA=BASE/'capture-2/partial-target-control'; DATA.mkdir()
spec=importlib.util.spec_from_file_location('first',ROOT/'wiki/drafts/measurements/compiler_parallelism_2026_10_28/sweep.py')
first=importlib.util.module_from_spec(spec); spec.loader.exec_module(first)
first.EVIDENCE=DATA/'logs'; first.EVIDENCE.mkdir()
seed=json.loads((BASE/'baseline.json').read_text(encoding='utf-8'))
producers={'baseline':seed['producer'],'candidate':json.loads((BASE/'candidate/producer.json').read_text(encoding='utf-8'))['producer']}
expected=next(r for r in seed['observations'] if r['subject']=='stack32')['artifacts']
results={'producers':producers,'observations':[],'refused_target':'macos_arm64'}
for flavor,producer in producers.items():
    assert first.require_optimized(producer['binary'])['sha256']==producer['sha256']
    output=DATA/flavor; output.mkdir()
    (output/'macos_arm64').write_text('intentional target-directory obstruction\n',encoding='utf-8')
    cache=Path(os.environ['LOCALAPPDATA'])/('OmegaPerf-partial-'+uuid.uuid4().hex[:12]); cache.mkdir()
    env=dict(os.environ,LOCALAPPDATA=str(cache),OMEGA_LIBRARY_ROOT=str(ROOT/'source/library'),RAYON_NUM_THREADS='32'); env.pop('OMEGA_PHASE_PROFILE',None)
    command=[producer['binary'],'--timings','--build-dir',str(output)]
    for target in ('windows_x86_64','macos_arm64','linux_x86_64','linux_arm64','windows_x86_64'): command+=['--target',target]
    command.append(str(ROOT/first.SUBJECTS['stack32']))
    row,text=first.capture(command,flavor,env); row['flavor']=flavor
    results['observations'].append(row)
    (DATA/'results.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
    assert row['exit']==1 and 'target macos_arm64 failed' in text
    assert re.findall(r'^target (\w+):$',text,re.M)==seed['targets']
    paths=re.findall(r'^published native output to (.+)$',text,re.M); assert len(paths)==3
    artifacts={}
    for path in paths:
        artifact=Path(path.strip()); target=next(t for t in seed['targets'] if t in artifact.parts)
        assert target!='macos_arm64' and target not in artifacts
        assert artifact.is_file() and first.digest(artifact)==expected[target]['sha256']
        artifacts[target]={'path':str(artifact),'sha256':first.digest(artifact)}
        if target=='windows_x86_64':
            native,_=first.capture([str(artifact)],flavor+'-native',dict(os.environ)); assert native['exit']==0; row['native']=native
    assert set(artifacts)==set(seed['targets'])-{'macos_arm64'}
    row['artifacts']=artifacts
    (DATA/'results.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
    print('partial target failure/order/duplicate control',flavor,'passed',flush=True)
results['complete']=True
(DATA/'results.json').write_text(json.dumps(results,indent=2)+'\n',encoding='utf-8')
